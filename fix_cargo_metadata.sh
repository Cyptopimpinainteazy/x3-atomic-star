#!/bin/bash
SOURCE_REPO="/home/lojak/Desktop/X3_RECOVER_RC1"
SOURCE_COMMIT="884e33d4513a"
MAX_ITERATIONS=120
RESTORED_DIRS=()
SUCCESS=false
FINAL_ERROR=""
UNRESOLVED_MANIFEST=""

for ((i=1; i<=MAX_ITERATIONS; i++)); do
    echo "Iteration $i..."
    # Run cargo metadata and capture stderr
    ERROR_OUTPUT=$(cargo metadata --no-deps 2>&1 > /dev/null)
    EXIT_CODE=$?

    if [ $EXIT_CODE -eq 0 ]; then
        SUCCESS=true
        break
    fi

    # Try to extract the manifest path
    # Look for: error: failed to read `/path/to/Cargo.toml`
    MANIFEST_PATH=$(echo "$ERROR_OUTPUT" | grep -oP "failed to read \`?\K/[^ \`']+" | head -n 1 | sed 's/`$//')

    if [ -z "$MANIFEST_PATH" ]; then
        # Check if it was without absolute path
        MANIFEST_PATH=$(echo "$ERROR_OUTPUT" | grep -oP "failed to read \`?\K[^ \`']+" | head -n 1 | sed 's/`$//')
    fi

    if [ -z "$MANIFEST_PATH" ]; then
        FINAL_ERROR="$ERROR_OUTPUT"
        break
    fi

    # Make absolute path relative (assuming it starts with current dir)
    CURRENT_DIR=$(pwd)
    RELATIVE_PATH=${MANIFEST_PATH#"$CURRENT_DIR/"}
    
    # If path is still absolute and doesn't belong to current dir, we might have a problem
    if [[ "$RELATIVE_PATH" == /* ]]; then
         FINAL_ERROR="$ERROR_OUTPUT"
         UNRESOLVED_MANIFEST="$MANIFEST_PATH"
         break
    fi

    # The directory to restore is the parent of the Cargo.toml
    DIR_TO_RESTORE=$(dirname "$RELATIVE_PATH")

    # Check if it exists in the source repo
    if [ -e "$SOURCE_REPO/$RELATIVE_PATH" ]; then
        echo "Restoring $DIR_TO_RESTORE from $SOURCE_REPO at $SOURCE_COMMIT"
        # We need to run git checkout from the source repo but target the current working directory? 
        # No, "git checkout SOURCE_COMMIT -- <dir>" usually means restoring a file/dir into the current repo from its own history.
        # But wait, the instructions imply restoring from SOURCE_REPO. Usually this means:
        # git --git-dir=$SOURCE_REPO/.git checkout $SOURCE_COMMIT -- $DIR_TO_RESTORE
        
        if git --git-dir="$SOURCE_REPO/.git" show "$SOURCE_COMMIT:$DIR_TO_RESTORE" > /dev/null 2>&1; then
             # This command restores paths from the source repo to the current index and working tree
             # However, git checkout FROM ANOTHER REPO usually involves adding it as a remote or using object files.
             # Given the context, perhaps the user meant copying the dir or the files exist in the current repo's history at that commit?
             # "restore its parent dir via git checkout SOURCE_COMMIT -- <dir>"
             # This syntax implies SOURCE_COMMIT is in the CURRENT repository.
             
             if git checkout "$SOURCE_COMMIT" -- "$DIR_TO_RESTORE" 2>&1; then
                 RESTORED_DIRS+=("$DIR_TO_RESTORE")
             else
                 FINAL_ERROR="$ERROR_OUTPUT"
                 UNRESOLVED_MANIFEST="$MANIFEST_PATH"
                 echo "Git checkout failed for $DIR_TO_RESTORE"
                 break
             fi
        else
            FINAL_ERROR="$ERROR_OUTPUT"
            UNRESOLVED_MANIFEST="$MANIFEST_PATH"
            echo "Dir $DIR_TO_RESTORE not found in source commit $SOURCE_COMMIT"
            break
        fi
    else
        FINAL_ERROR="$ERROR_OUTPUT"
        UNRESOLVED_MANIFEST="$MANIFEST_PATH"
        echo "Manifest $RELATIVE_PATH not found in source repository"
        break
    fi
done

echo "--- RESULTS ---"
if [ "$SUCCESS" = true ]; then
    echo "Status: PASSED"
else
    echo "Status: FAILED"
    echo "Unresolved Manifest: $UNRESOLVED_MANIFEST"
    echo "Error Output (first 20 lines):"
    echo "$FINAL_ERROR" | head -n 20
fi
echo "Restored Directories:"
for dir in "${RESTORED_DIRS[@]}"; do
    echo "$dir"
done
