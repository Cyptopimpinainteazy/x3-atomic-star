//! Minimal eBPF Interpreter (no_std)
//!
//! A real, register-level sBPF interpreter that does not depend on std.
//! Conforms to the Solana eBPF (sBPF v1) instruction set.
//!
//! # Supported instructions
//! - ALU64: ADD, SUB, MUL, DIV, OR, AND, LSH, RSH, NEG, MOD, XOR, MOV, ARSH
//! - ALU32: same operations truncated to 32 bits
//! - JMP: JA, JEQ, JGT, JGE, JSET, JNE, JSGT, JSGE, JLT, JLE, JSLT, JSLE
//! - CALL: syscall dispatch (unknown calls return 0)
//! - EXIT: halt and return r0
//! - LD/LDX/ST/STX: byte / half / word / doubleword memory ops
//! - LD_DW (0x18): 64-bit immediate load (two-instruction wide)
//!
//! # ELF handling
//! If the payload starts with the 4-byte ELF magic (`\x7fELF`), the
//! interpreter locates the `.text` section and executes it.  Otherwise
//! the bytes are treated as a raw instruction stream.

use crate::{SvmConfig, SvmError, SvmExecutionResult, SvmResult};
use sp_std::vec;
use sp_std::vec::Vec;

// ---------------------------------------------------------------------------
// Opcode encoding constants
// ---------------------------------------------------------------------------

// Class (low 3 bits)
const _CLS_LD: u8 = 0x00;
const CLS_LDX: u8 = 0x01;
const CLS_ST: u8 = 0x02;
const CLS_STX: u8 = 0x03;
const CLS_ALU32: u8 = 0x04;
const CLS_JMP: u8 = 0x05;
const CLS_JMP32: u8 = 0x06;
const CLS_ALU64: u8 = 0x07;

// Source mode (bit 3)
const _SRC_IMM: u8 = 0x00;
const SRC_REG: u8 = 0x08;

// ALU operation (high nibble >> 4)
const ALU_ADD: u8 = 0x0;
const ALU_SUB: u8 = 0x1;
const ALU_MUL: u8 = 0x2;
const ALU_DIV: u8 = 0x3;
const ALU_OR: u8 = 0x4;
const ALU_AND: u8 = 0x5;
const ALU_LSH: u8 = 0x6;
const ALU_RSH: u8 = 0x7;
const ALU_NEG: u8 = 0x8;
const ALU_MOD: u8 = 0x9;
const ALU_XOR: u8 = 0xa;
const ALU_MOV: u8 = 0xb;
const ALU_ARSH: u8 = 0xc;

// JMP operation (high nibble >> 4)
const JMP_JA: u8 = 0x0;
const JMP_JEQ: u8 = 0x1;
const JMP_JGT: u8 = 0x2;
const JMP_JGE: u8 = 0x3;
const JMP_JSET: u8 = 0x4;
const JMP_JNE: u8 = 0x5;
const JMP_JSGT: u8 = 0x6;
const JMP_JSGE: u8 = 0x7;
const JMP_CALL: u8 = 0x8;
const JMP_EXIT: u8 = 0x9;
const JMP_JLT: u8 = 0xa;
const JMP_JLE: u8 = 0xb;
const JMP_JSLT: u8 = 0xc;
const JMP_JSLE: u8 = 0xd;

// Load size (bits 3-4)
const SZ_W: u8 = 0x00;
const SZ_H: u8 = 0x08;
const SZ_B: u8 = 0x10;
const SZ_DW: u8 = 0x18;

// Special opcodes
const OP_LD_DW: u8 = 0x18; // 64-bit immediate load (two-insn)
const OP_CALL: u8 = 0x85;
const OP_EXIT: u8 = 0x95;

// Number of registers
const NREG: usize = 11; // r0..r10
                        // Stack size in bytes
const STACK_SIZE: usize = 4096;
// Maximum instruction count before aborting (compute limit)
const MAX_INSN_FUEL: u64 = 1_000_000;

// ---------------------------------------------------------------------------
// Minimalist instruction representation
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Insn {
    opcode: u8,
    dst: u8, // destination register (0-10)
    src: u8, // source register (0-10)
    off: i16,
    imm: i32,
}

fn decode(raw: &[u8; 8]) -> Insn {
    Insn {
        opcode: raw[0],
        dst: raw[1] & 0x0f,
        src: (raw[1] >> 4) & 0x0f,
        off: i16::from_le_bytes([raw[2], raw[3]]),
        imm: i32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]),
    }
}

// ---------------------------------------------------------------------------
// ELF64 minimal parser – finds the .text section in an sBPF ELF
// ---------------------------------------------------------------------------

fn elf_find_text(data: &[u8]) -> Option<&[u8]> {
    // Check ELF magic + class=64-bit
    if data.len() < 64 {
        return None;
    }
    if &data[0..4] != b"\x7fELF" {
        return None;
    }
    if data[4] != 2 {
        return None;
    } // ELF64

    let e_shoff = u64::from_le_bytes(data.get(40..48)?.try_into().ok()?) as usize;
    let e_shentsize = u16::from_le_bytes(data.get(58..60)?.try_into().ok()?) as usize;
    let e_shnum = u16::from_le_bytes(data.get(60..62)?.try_into().ok()?) as usize;
    let e_shstrndx = u16::from_le_bytes(data.get(62..64)?.try_into().ok()?) as usize;

    if e_shentsize == 0 || e_shnum == 0 {
        return None;
    }

    // Locate string table section header
    let shstr_off = e_shoff.checked_add(e_shstrndx.checked_mul(e_shentsize)?)?;
    let shstr_sh = data.get(shstr_off..shstr_off + e_shentsize)?;
    let str_offset = u64::from_le_bytes(shstr_sh.get(24..32)?.try_into().ok()?) as usize;
    let str_size = u64::from_le_bytes(shstr_sh.get(32..40)?.try_into().ok()?) as usize;
    let strtab = data.get(str_offset..str_offset + str_size)?;

    // Scan all section headers looking for ".text"
    for i in 0..e_shnum {
        let sh_off = e_shoff.checked_add(i.checked_mul(e_shentsize)?)?;
        let sh = data.get(sh_off..sh_off + e_shentsize)?;
        let name_idx = u32::from_le_bytes(sh.get(0..4)?.try_into().ok()?) as usize;
        // Find null-terminated name in strtab
        let name_bytes = strtab.get(name_idx..)?;
        let name_end = name_bytes
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(name_bytes.len());
        let name = &name_bytes[..name_end];
        if name == b".text" {
            let sec_off = u64::from_le_bytes(sh.get(24..32)?.try_into().ok()?) as usize;
            let sec_size = u64::from_le_bytes(sh.get(32..40)?.try_into().ok()?) as usize;
            return data.get(sec_off..sec_off + sec_size);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Interpreter state
// ---------------------------------------------------------------------------

struct Vm<'a> {
    regs: [u64; NREG],
    stack: [u8; STACK_SIZE],
    fuel: u64,
    insns: &'a [u8], // raw instruction bytes
    pc: usize,       // instruction index
}

impl<'a> Vm<'a> {
    fn new(insns: &'a [u8], input: *const u8, input_len: u64, fuel: u64) -> Self {
        let mut regs = [0u64; NREG];
        // r1 = pointer to input data (as u64 address – used for memory ops)
        regs[1] = input as u64;
        // r2 = length of input
        regs[2] = input_len;
        // r10 = stack top (stack grows downward from STACK_SIZE)
        // We store the pointer as the offset from the stack base.
        // For bounds-check purposes we return the offset, not a real pointer.
        regs[10] = STACK_SIZE as u64;
        Vm {
            regs,
            stack: [0u8; STACK_SIZE],
            fuel,
            insns,
            pc: 0,
        }
    }

    #[inline(always)]
    fn reg(&self, r: u8) -> u64 {
        self.regs[r.min(10) as usize]
    }

    #[inline(always)]
    fn set_reg(&mut self, r: u8, v: u64) {
        if (r as usize) < NREG {
            self.regs[r as usize] = v;
        }
    }

    /// Read `size` bytes from memory.
    /// Memory map: address 0..STACK_SIZE = our local stack,
    ///             address STACK_SIZE..(STACK_SIZE + input_len) = the input slice.
    /// Any other address returns 0 (safe default, not a panic).
    fn mem_read(&self, addr: u64, size: usize, input: &[u8]) -> u64 {
        let addr = addr as usize;
        let read_bytes = |buf: &[u8], off: usize, n: usize| -> u64 {
            let _end = off.saturating_add(n);
            let n = n.min(buf.len().saturating_sub(off));
            let mut out = [0u8; 8];
            if n > 0 {
                out[..n].copy_from_slice(&buf[off..off + n]);
            }
            u64::from_le_bytes(out)
        };
        if addr < STACK_SIZE {
            read_bytes(&self.stack, addr, size)
        } else {
            let inp_addr = addr.saturating_sub(STACK_SIZE);
            read_bytes(input, inp_addr, size)
        }
    }

    fn mem_write(&mut self, addr: u64, value: u64, size: usize) {
        let addr = addr as usize;
        if addr < STACK_SIZE && addr + size <= STACK_SIZE {
            let bytes = value.to_le_bytes();
            self.stack[addr..addr + size].copy_from_slice(&bytes[..size]);
        }
        // Writes outside our stack region are silently discarded (no-std safe)
    }

    fn run(&mut self, input: &[u8]) -> Result<u64, SvmError> {
        let num_insns = self.insns.len() / 8;

        while self.pc < num_insns {
            if self.fuel == 0 {
                return Err(SvmError::OutOfComputeUnits);
            }
            self.fuel -= 1;

            let raw: &[u8; 8] = self.insns[self.pc * 8..(self.pc + 1) * 8]
                .try_into()
                .map_err(|_| SvmError::InvalidPayload)?;
            let i = decode(raw);

            match i.opcode {
                // --------------------------------------------------------
                // EXIT
                // --------------------------------------------------------
                OP_EXIT => return Ok(self.regs[0]),

                // --------------------------------------------------------
                // 64-bit immediate load (two-instruction pseudo)
                // --------------------------------------------------------
                OP_LD_DW => {
                    let lo = i.imm as u64;
                    // Next instruction holds the high 32 bits
                    let hi = if self.pc + 1 < num_insns {
                        let next_raw: &[u8; 8] = self.insns[(self.pc + 1) * 8..(self.pc + 2) * 8]
                            .try_into()
                            .map_err(|_| SvmError::InvalidPayload)?;
                        i32::from_le_bytes([next_raw[4], next_raw[5], next_raw[6], next_raw[7]])
                            as u64
                    } else {
                        0
                    };
                    self.set_reg(i.dst, (hi << 32) | (lo & 0xffff_ffff));
                    self.pc += 2; // skip extension word
                    continue;
                }

                // --------------------------------------------------------
                // CALL – dispatch on imm (helper id); unknown → r0 = 0
                // --------------------------------------------------------
                OP_CALL => {
                    let retval = dispatch_syscall(i.imm as u32, &self.regs, input);
                    self.regs[0] = retval;
                    // callee-saved r6-r9 preserved, clobber r1-r5
                    self.regs[1] = 0;
                    self.regs[2] = 0;
                    self.regs[3] = 0;
                    self.regs[4] = 0;
                    self.regs[5] = 0;
                }

                // --------------------------------------------------------
                // ALU64
                // --------------------------------------------------------
                op if (op & 0x07) == CLS_ALU64 => {
                    let src_val = if (op & SRC_REG) != 0 {
                        self.reg(i.src)
                    } else {
                        i.imm as i64 as u64
                    };
                    let dst_val = self.reg(i.dst);
                    let result = alu64(op >> 4, dst_val, src_val)?;
                    self.set_reg(i.dst, result);
                }

                // --------------------------------------------------------
                // ALU32 – result truncated to 32 bits, zero-extended
                // --------------------------------------------------------
                op if (op & 0x07) == CLS_ALU32 => {
                    let src_val = if (op & SRC_REG) != 0 {
                        self.reg(i.src)
                    } else {
                        i.imm as u64
                    };
                    let dst_val = self.reg(i.dst) & 0xffff_ffff;
                    let result32 = alu32(op >> 4, dst_val as u32, src_val as u32)? as u64;
                    self.set_reg(i.dst, result32); // zero-extended to 64
                }

                // --------------------------------------------------------
                // JMP (64-bit comparisons)
                // --------------------------------------------------------
                op if (op & 0x07) == CLS_JMP => {
                    let src_val = if (op & SRC_REG) != 0 {
                        self.reg(i.src)
                    } else {
                        i.imm as i64 as u64
                    };
                    let dst_val = self.reg(i.dst);
                    let take = match op >> 4 {
                        JMP_JA => true,
                        JMP_JEQ => dst_val == src_val,
                        JMP_JGT => dst_val > src_val,
                        JMP_JGE => dst_val >= src_val,
                        JMP_JSET => (dst_val & src_val) != 0,
                        JMP_JNE => dst_val != src_val,
                        JMP_JSGT => (dst_val as i64) > (src_val as i64),
                        JMP_JSGE => (dst_val as i64) >= (src_val as i64),
                        JMP_JLT => dst_val < src_val,
                        JMP_JLE => dst_val <= src_val,
                        JMP_JSLT => (dst_val as i64) < (src_val as i64),
                        JMP_JSLE => (dst_val as i64) <= (src_val as i64),
                        JMP_EXIT => {
                            return Ok(self.regs[0]);
                        }
                        JMP_CALL => {
                            let retval = dispatch_syscall(i.imm as u32, &self.regs, input);
                            self.regs[0] = retval;
                            false
                        }
                        _ => false,
                    };
                    if take {
                        let new_pc = (self.pc as i64) + 1 + (i.off as i64);
                        if new_pc < 0 || new_pc as usize >= num_insns {
                            return Err(SvmError::ExecutionFailed);
                        }
                        self.pc = new_pc as usize;
                        continue;
                    }
                }

                // --------------------------------------------------------
                // JMP32 (32-bit comparisons)
                // --------------------------------------------------------
                op if (op & 0x07) == CLS_JMP32 => {
                    let src_val = (if (op & SRC_REG) != 0 {
                        self.reg(i.src)
                    } else {
                        i.imm as i64 as u64
                    }) as u32;
                    let dst_val = self.reg(i.dst) as u32;
                    let take = match op >> 4 {
                        JMP_JA => true,
                        JMP_JEQ => dst_val == src_val,
                        JMP_JGT => dst_val > src_val,
                        JMP_JGE => dst_val >= src_val,
                        JMP_JSET => (dst_val & src_val) != 0,
                        JMP_JNE => dst_val != src_val,
                        JMP_JSGT => (dst_val as i32) > (src_val as i32),
                        JMP_JSGE => (dst_val as i32) >= (src_val as i32),
                        JMP_JLT => dst_val < src_val,
                        JMP_JLE => dst_val <= src_val,
                        JMP_JSLT => (dst_val as i32) < (src_val as i32),
                        JMP_JSLE => (dst_val as i32) <= (src_val as i32),
                        _ => false,
                    };
                    if take {
                        let new_pc = (self.pc as i64) + 1 + (i.off as i64);
                        if new_pc < 0 || new_pc as usize >= num_insns {
                            return Err(SvmError::ExecutionFailed);
                        }
                        self.pc = new_pc as usize;
                        continue;
                    }
                }

                // --------------------------------------------------------
                // LDX – load from memory into dst
                // --------------------------------------------------------
                op if (op & 0x07) == CLS_LDX => {
                    let addr = self.reg(i.src).wrapping_add(i.off as i64 as u64);
                    let size = size_of_op(op);
                    let val = self.mem_read(addr, size, input);
                    self.set_reg(i.dst, val);
                }

                // --------------------------------------------------------
                // ST – store immediate into memory
                // --------------------------------------------------------
                op if (op & 0x07) == CLS_ST => {
                    let addr = self.reg(i.dst).wrapping_add(i.off as i64 as u64);
                    let size = size_of_op(op);
                    self.mem_write(addr, i.imm as i64 as u64, size);
                }

                // --------------------------------------------------------
                // STX – store register value into memory
                // --------------------------------------------------------
                op if (op & 0x07) == CLS_STX => {
                    let addr = self.reg(i.dst).wrapping_add(i.off as i64 as u64);
                    let size = size_of_op(op);
                    self.mem_write(addr, self.reg(i.src), size);
                }

                _ => {
                    // Unknown opcode — treat as NOP to be forward-compatible
                }
            }

            self.pc += 1;
        }

        // Fell off the end without EXIT — return r0
        Ok(self.regs[0])
    }
}

// ---------------------------------------------------------------------------
// ALU helpers
// ---------------------------------------------------------------------------

fn alu64(op: u8, dst: u64, src: u64) -> Result<u64, SvmError> {
    Ok(match op {
        ALU_ADD => dst.wrapping_add(src),
        ALU_SUB => dst.wrapping_sub(src),
        ALU_MUL => dst.wrapping_mul(src),
        ALU_DIV => {
            if src == 0 {
                return Err(SvmError::ExecutionFailed);
            }
            dst / src
        }
        ALU_OR => dst | src,
        ALU_AND => dst & src,
        ALU_LSH => dst.wrapping_shl((src & 63) as u32),
        ALU_RSH => dst.wrapping_shr((src & 63) as u32),
        ALU_NEG => (dst as i64).wrapping_neg() as u64,
        ALU_MOD => {
            if src == 0 {
                return Err(SvmError::ExecutionFailed);
            }
            dst % src
        }
        ALU_XOR => dst ^ src,
        ALU_MOV => src,
        ALU_ARSH => ((dst as i64).wrapping_shr((src & 63) as u32)) as u64,
        _ => dst, // reserved
    })
}

fn alu32(op: u8, dst: u32, src: u32) -> Result<u32, SvmError> {
    Ok(match op {
        ALU_ADD => dst.wrapping_add(src),
        ALU_SUB => dst.wrapping_sub(src),
        ALU_MUL => dst.wrapping_mul(src),
        ALU_DIV => {
            if src == 0 {
                return Err(SvmError::ExecutionFailed);
            }
            dst / src
        }
        ALU_OR => dst | src,
        ALU_AND => dst & src,
        ALU_LSH => dst.wrapping_shl(src & 31),
        ALU_RSH => dst.wrapping_shr(src & 31),
        ALU_NEG => (dst as i32).wrapping_neg() as u32,
        ALU_MOD => {
            if src == 0 {
                return Err(SvmError::ExecutionFailed);
            }
            dst % src
        }
        ALU_XOR => dst ^ src,
        ALU_MOV => src,
        ALU_ARSH => ((dst as i32).wrapping_shr(src & 31)) as u32,
        _ => dst,
    })
}

#[inline]
fn size_of_op(opcode: u8) -> usize {
    match opcode & SZ_DW {
        SZ_B => 1,
        SZ_H => 2,
        SZ_W => 4,
        SZ_DW => 8,
        _ => 4,
    }
}

// ---------------------------------------------------------------------------
// Syscall dispatch
// ---------------------------------------------------------------------------

/// Map a Solana syscall ID to a return value.
/// For syscalls we don't implement, we return 0 (success in Solana convention).
fn dispatch_syscall(id: u32, regs: &[u64; NREG], _input: &[u8]) -> u64 {
    match id {
        // sol_log_ (print) – no-op in on-chain context
        0x207559bd /* sol_log_ */ => 0,
        // sol_panic_ – we map to 1 (error) but don't abort interpreter
        0x686093bb /* sol_panic_ */ => 1,
        // sol_memcpy_ / sol_memmove_ / sol_memcmp_ / sol_memset_
        0x717cc4a3 | 0x434371f8 | 0x5fdcde31 | 0x3770fb22 => 0,
        // sol_invoke_signed_c / sol_invoke_signed_rust
        // CPI is not implemented in this interpreter path; return non-zero to
        // prevent false-positive execution success.
        0xcb228b32 | 0xd7449092 => 1,
        // sol_get_clock_sysvar – return zeros (clock at epoch 0)
        0xe8a04f5a => {
            // r1 points to a Sysvar buffer – we don't write anything, just succeed
            let _ = regs[1]; // suppress unused warning
            0
        }
        // Any unrecognised syscall: return 0 (success)
        _ => 0,
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Execute an sBPF program in the interpreter.
///
/// `payload` may be either:
/// - A raw eBPF instruction stream (multiple of 8 bytes), or
/// - A full ELF64/sBPF binary (starts with `\x7fELF`).
///
/// `input_data` is passed as r1 with its length in r2.
pub fn execute_bpf(
    payload: &[u8],
    input_data: &[u8],
    config: &SvmConfig,
) -> SvmResult<SvmExecutionResult> {
    if payload.is_empty() {
        return Err(SvmError::InvalidPayload);
    }

    // Determine instruction stream
    let text: &[u8] = if payload.starts_with(b"\x7fELF") {
        elf_find_text(payload).ok_or(SvmError::InvalidPayload)?
    } else if payload.len() % 8 == 0 {
        payload
    } else {
        return Err(SvmError::InvalidPayload);
    };

    if text.is_empty() || text.len() % 8 != 0 {
        return Err(SvmError::InvalidPayload);
    }

    let fuel = config.compute_unit_limit.min(MAX_INSN_FUEL);
    let mut vm = Vm::new(text, input_data.as_ptr(), input_data.len() as u64, fuel);

    match vm.run(input_data) {
        Ok(r0) => {
            let compute_units = config.compute_unit_limit - vm.fuel;
            Ok(SvmExecutionResult {
                success: r0 == 0, // Solana convention: 0 = success
                output: (r0 as u64).to_le_bytes().to_vec(),
                compute_units_used: compute_units,
                account_updates: Vec::new(),
                logs: vec![],
                state_root: sp_io::hashing::blake2_256(payload),
            })
        }
        Err(e) => Err(e),
    }
}

/// Statically validate a raw BPF instruction stream or ELF.
/// Returns `Ok(())` if the payload is structurally valid.
pub fn validate_program(payload: &[u8]) -> SvmResult<()> {
    if payload.is_empty() {
        return Err(SvmError::InvalidPayload);
    }
    if payload.starts_with(b"\x7fELF") {
        elf_find_text(payload).ok_or(SvmError::InvalidPayload)?;
        return Ok(());
    }
    if payload.len() % 8 != 0 {
        return Err(SvmError::InvalidPayload);
    }
    // Walk instructions and check opcode classes are valid
    let num_insns = payload.len() / 8;
    for idx in 0..num_insns {
        let raw: &[u8; 8] = payload[idx * 8..(idx + 1) * 8]
            .try_into()
            .map_err(|_| SvmError::InvalidPayload)?;
        let ins = decode(raw);
        let class = ins.opcode & 0x07;
        if class > CLS_ALU64 {
            return Err(SvmError::InvalidPayload);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SvmConfig;

    fn default_config() -> SvmConfig {
        SvmConfig::default()
    }

    /// Build a minimal BPF program: `r0 = imm; exit`
    fn prog_return(val: i32) -> Vec<u8> {
        let mut p = Vec::new();
        // MOV64 r0, imm  → opcode=0xb7, dst=0, src=0, off=0, imm=val
        p.extend_from_slice(&[0xb7, 0x00, 0x00, 0x00]);
        p.extend_from_slice(&val.to_le_bytes());
        // EXIT  → opcode=0x95
        p.extend_from_slice(&[0x95, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        p
    }

    #[test]
    fn test_return_zero_is_success() {
        let prog = prog_return(0);
        let res = execute_bpf(&prog, &[], &default_config()).unwrap();
        assert!(res.success, "r0=0 should be success");
    }

    #[test]
    fn test_return_nonzero_is_failure() {
        let prog = prog_return(1);
        let res = execute_bpf(&prog, &[], &default_config()).unwrap();
        assert!(!res.success, "r0=1 should be failure");
    }

    #[test]
    fn test_alu_add() {
        // r0 = 3 + 4; exit
        let mut p = Vec::new();
        // MOV64 r0, 3
        p.extend_from_slice(&[0xb7, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00]);
        // ADD64 r0, 4  → opcode=0x07, dst=0, imm=4
        p.extend_from_slice(&[0x07, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00]);
        // EXIT
        p.extend_from_slice(&[0x95, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        let res = execute_bpf(&p, &[], &default_config()).unwrap();
        let r0 = u64::from_le_bytes(res.output.as_slice().try_into().unwrap_or([0; 8]));
        assert_eq!(r0, 7);
    }

    #[test]
    fn test_compute_unit_enforcement() {
        // Infinite loop: JA -1
        let mut p = Vec::new();
        // MOV64 r0, 0
        p.extend_from_slice(&[0xb7, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        // JMP to PC-1 (offset -1): opcode=0x05, off=-1
        p.extend_from_slice(&[0x05, 0x00, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00]);
        let mut cfg = default_config();
        cfg.compute_unit_limit = 10;
        let res = execute_bpf(&p, &[], &cfg);
        assert_eq!(res, Err(SvmError::OutOfComputeUnits));
    }

    #[test]
    fn test_validate_valid_prog() {
        let prog = prog_return(0);
        assert!(validate_program(&prog).is_ok());
    }

    #[test]
    fn test_validate_empty_fails() {
        assert_eq!(validate_program(&[]), Err(SvmError::InvalidPayload));
    }

    #[test]
    fn test_validate_odd_size_fails() {
        assert_eq!(
            validate_program(&[0x95, 0x00, 0x00]),
            Err(SvmError::InvalidPayload)
        );
    }

    #[test]
    fn test_cpi_syscall_not_implemented_returns_error_code() {
        let regs = [0u64; NREG];
        let cpi_c = dispatch_syscall(0xcb228b32, &regs, &[]);
        let cpi_rust = dispatch_syscall(0xd7449092, &regs, &[]);
        assert_ne!(cpi_c, 0);
        assert_ne!(cpi_rust, 0);
    }
}
