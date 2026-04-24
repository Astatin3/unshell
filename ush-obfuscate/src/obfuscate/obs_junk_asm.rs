use proc_macro::TokenStream;
use quote::quote;
use rand::rngs::{SmallRng, StdRng};
use rand::{Rng, RngExt, SeedableRng};
use syn::{LitFloat, parse_macro_input};

// const MIN_TAGS: u32 = 1; // Maximum instructions per recursive block
// const MAX_TAGS: u32 = 22; // Maximum instructions per recursive block

// const MIN_INSTRUCTIONS: u32 = 1; // Maximum instructions per recursive block
// const MAX_INSTRUCTIONS: u32 = 22; // Maximum instructions per recursive block

// const MIN_JUMPS: u32 = 1;
// const MAX_JUMPS: u32 = 5;

const CHAIN_WEIGHT: f64 = 1.0;
const TAG_WEIGHT: f64 = 1.0;
const INST_WEIGHT: f64 = 3.0;
const JUMP_WEIGHT: f64 = 2.0;

// The full list of 64-bit registers in AT&T syntax (used by default in asm!)
const REGISTERS: &[&str] = &[
    "%rax", "%rbx", "%rcx", "%rdx", "%rsi", "%rdi", "%r8", "%r9", "%r10", "%r11", "%r12", "%r13",
    "%r14", "%r15",
];

// Conditional Jumps in AT&T syntax.
const COND_JUMPS: &[&str] = &[
    "je", "jne", "jg", "jge", "jl", "jle", "ja", "jnb", "jc", "jnc", "jz", "jnz",
];

// Arithmetic/Logic operations with the 'q' (quad-word) suffix.
const ARITHITHMETIC_OPS: &[&str] = &["addq", "subq", "xorq", "andq", "orq"];

// --- Helper Functions for Modular Generation ---

/// Generates a unique label name for the given depth and ID.
fn generate_label(prefix: &str, depth: usize, id: usize) -> String {
    format!(".L_{}_{}_{}", prefix, depth, id)
}
/// Generates a highly randomized, complex instruction using different addressing modes.
fn generate_complex_mutation(rng: &mut SmallRng) -> String {
    let op = ARITHITHMETIC_OPS[rng.random_range(0..ARITHITHMETIC_OPS.len())];

    match rng.random_range(0..3) {
        // Pattern 0: Register-Immediate
        // Example: "addq $0x1234, %rax"
        0 => {
            let reg = REGISTERS[rng.random_range(0..REGISTERS.len())];
            let immediate = rng.random_range(1..=0xFFFF);
            format!("\t{} ${}, {}", op, immediate, reg)
        }
        // Pattern 1: Register-Register
        // Example: "xorq %rbx, %rcx"
        1 => {
            let reg_src = REGISTERS[rng.random_range(0..REGISTERS.len())];
            let reg_dst = REGISTERS[rng.random_range(0..REGISTERS.len())];
            format!("\t{} {}, {}", op, reg_src, reg_dst)
        }
        // Pattern 2: LEA (Complex Address Calculation)
        // Example: "leaq (%rax, %rbx, 4), %rcx"
        2 => {
            let reg_base = REGISTERS[rng.random_range(0..REGISTERS.len())];
            let reg_index = REGISTERS[rng.random_range(0..REGISTERS.len())];
            let reg_dst = REGISTERS[rng.random_range(0..REGISTERS.len())];
            let scale = 1 << rng.random_range(0..4); // Scale is 1, 2, 4, or 8
            format!(
                "\tleaq ({}, {}, {}), {}",
                reg_base, reg_index, scale, reg_dst
            )
        }
        _ => String::new(), // Should not happen
    }
}

/// Generates a comparison followed by a conditional jump to a specific label.
fn generate_conditional_jump(rng: &mut SmallRng, label: &str) -> String {
    let reg1 = REGISTERS[rng.random_range(0..REGISTERS.len())];
    let reg2 = REGISTERS[rng.random_range(0..REGISTERS.len())];
    let jump = COND_JUMPS[rng.random_range(0..COND_JUMPS.len())];

    // Example: "cmpq %rdx, %rsi; jg .L_target_"
    format!("\tcmpq {}, {}; {} {}\n", reg1, reg2, jump, label)
}

// --- The Core DAG Recursive Algorithm ---

fn generate_dag_block(weight: f64, rng: &mut SmallRng, total_count: usize) -> String {
    let labels = (0..total_count)
        .map(|i| {
            (0..{
                let mut n = 1;
                while !rng.random_bool((weight.sqrt() * TAG_WEIGHT).min(1.)) {
                    n += 1;
                }
                n
            })
                .map(|j| generate_label("dag", i, j))
                .collect::<Vec<String>>()
        })
        // .flatten()
        .collect::<Vec<Vec<String>>>();

    let mut assembly_block = String::new();

    // 3. Instruction Loop and DAG construction
    for i in 0..total_count {
        let chain_labels = &labels[i];
        let num_labels = chain_labels.len();
        for j in 0..num_labels {
            let current_label = &chain_labels[j];
            assembly_block.push_str(&format!("{}:\n", current_label));

            let mut inst_count = 1;
            while !rng.random_bool((weight * INST_WEIGHT).min(1.)) {
                inst_count += 1;
            }

            for _ in 0..inst_count {
                assembly_block.push_str(&format!("{}\n", generate_complex_mutation(rng)));
            }

            // Conditional Forward Jump (Creates DAG edges)
            if i < total_count - 1 {
                let mut jump_count = 1;
                while !rng.random_bool((weight.sqrt() * JUMP_WEIGHT).min(1.)) {
                    jump_count += 1;
                }

                for _ in 0..jump_count {
                    // Jump to a random label strictly ahead of the current one
                    let target_chain = if j + 1 < num_labels {
                        rng.random_range(i..total_count)
                    } else {
                        rng.random_range(i + 1..total_count)
                    };
                    let chain_labels = &labels[target_chain];

                    let target_index = if target_chain == i {
                        rng.random_range((j + 1)..num_labels)
                    } else {
                        rng.random_range(0..chain_labels.len())
                    };

                    let target_label = &chain_labels[target_index];
                    assembly_block.push_str(&generate_conditional_jump(rng, target_label));
                }
                // }
            }
        }
    }

    // 4. Backward Conditional Jump (Adds controlled cycles)
    // Only at the end of the block, allowing a chance to loop back to an earlier instruction.
    // if num_labels > 1 && rng.random_bool(weight * 3.) {
    //     let target_index = rng.random_range(0..num_labels as usize - 1);
    //     let target_label = &labels[target_index];
    //     assembly_block.push_str(&format!("{}\n", generate_complex_mutation(rng)));
    //     assembly_block.push_str(&generate_conditional_jump(rng, target_label));
    //     assembly_block.push_str("// Backward Conditional Jump to maintain short execution\n");
    // }

    assembly_block
}

pub fn junk_asm(input: TokenStream) -> TokenStream {
    // 1. Parse the input (expecting an optional f32 weight)
    let weight: f64 = if input.is_empty() {
        None
    } else {
        match parse_macro_input!(input as LitFloat).base10_parse::<f64>() {
            Ok(w) => Some(1. / (w + 1.)), // Move weight variable to be more
            Err(_) => None,
        }
    }
    .expect("Expected F64");
    // let final_weight = input_weight.unwrap_or(WEIGHT);

    // 2. Setup
    let mut rng = SmallRng::from_seed(rand::random());

    let count = {
        let mut n = 1;
        while !rng.random_bool((weight.sqrt() * CHAIN_WEIGHT).min(1.)) {
            n += 1;
        }
        n
    };

    // eeeeeeeeeeee

    // 3. Generate Assembly
    // let main_assembly = (0..count)
    //     .map(|i| generate_dag_block(weight, &mut rng, i, count))
    //     .into_iter()
    //     .collect::<String>();

    let main_assembly = generate_dag_block(weight, &mut rng, count);

    // println!("{}", main_assembly);

    // 4. Wrap in `asm!`
    let expanded = quote! {
        // Output will replace the junk_asm!(...) call
        {
            // Execute the code using the standard `asm!` macro.
            unsafe {
                #[allow(named_asm_labels)]
                core::arch::asm!(
                    // The generated junk code
                    // Note: We MUST use AT&T syntax (e.g., %rax, $100) due to options(att_syntax)
                    // The code is generated in AT&T syntax.
                    #main_assembly,

                    // Pass the simulated external address into a temporary register (%r15)
                    // This allows instructions to reference an "external" scope using memory reads/writes.
                    // in(reg) external_addr_ref,

                    // Clobber all general-purpose registers to force saving/restoring
                    clobber_abi("sysv64"),

                    // Correct options for non-volatile junk code
                    options(att_syntax, nomem, nostack, preserves_flags)
                );
            }
        }
    };

    expanded.into()
}

// NOTE: To make the example runnable, the `src/main.rs` file would now call
// junk_asm!(0.2) or junk_asm!(). The instruction sizes and jump structure
// are now compliant with your requirements.
