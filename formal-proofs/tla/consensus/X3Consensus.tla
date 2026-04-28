---------------------------- MODULE X3Consensus ----------------------------
EXTENDS Naturals, FiniteSets

CONSTANTS Validators, MaxHeight, Quorum

VARIABLES finalized, votes

VoteSet == [validator: Validators, height: 0..MaxHeight, block: 0..MaxHeight]

TypeOK ==
    /\ finalized \in [0..MaxHeight -> 0..MaxHeight \cup {-1}]
    /\ votes \in SUBSET VoteSet

Init ==
    /\ finalized = [h \in 0..MaxHeight |-> -1]
    /\ votes = {}

CastVote(v, h, b) ==
    /\ v \in Validators
    /\ h \in 0..MaxHeight
    /\ b \in 0..MaxHeight
    /\ votes' = votes \cup {[validator |-> v, height |-> h, block |-> b]}
    /\ UNCHANGED finalized

Finalize(h, b) ==
    /\ h \in 0..MaxHeight
    /\ b \in 0..MaxHeight
    /\ Cardinality({x \in votes : x.height = h /\ x.block = b}) >= Quorum
    /\ finalized[h] = -1
    /\ finalized' = [finalized EXCEPT ![h] = b]
    /\ UNCHANGED votes

Next == \E v, h, b : CastVote(v, h, b) \/ Finalize(h, b)

NoFork ==
    \A h \in 0..MaxHeight:
        finalized[h] # -1 =>
            \A b \in 0..MaxHeight:
                (Cardinality({x \in votes : x.height = h /\ x.block = b}) >= Quorum) =>
                    b = finalized[h]

Spec == Init /\ [][Next]_<<finalized, votes>>

THEOREM Spec => []TypeOK
THEOREM Spec => []NoFork

=============================================================================
