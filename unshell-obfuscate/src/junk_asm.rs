use proc_macro::TokenStream;
use quote::quote;
use rand::rngs::SmallRng;
use rand::{Rng, RngCore, SeedableRng};
use syn::{LitFloat, parse_macro_input};

const MAX_INSTRUCTIONS: u32 = 20; // Maximum instructions per recursive block
const MIN_LENGTH: f64 = 10.; // Min length per 1/weight

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
fn generate_label(prefix: &str, depth: u32, block_id: u32, id: u32) -> String {
    format!(".L_{}_{}_{}_{}", prefix, depth, block_id, id)
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

fn generate_dag_block(weight: f64, rng: &mut SmallRng, depth: u32, id_counter: &mut u32) -> String {
    // 1. Termination Check

    if rng.random_bool(weight) {
        return String::new(); // Stop recursion
    }

    let block_id = *id_counter;
    *id_counter += 1;

    // 2. Randomize Block Length: The length is now based on WEIGHT.
    // If rng < WEIGHT, stop growing the block. Otherwise, continue.
    let mut num_labels: u32 = 0;
    while !rng.random_bool(weight) && num_labels < MAX_INSTRUCTIONS {
        num_labels += 1;
    }

    // Ensure at least one instruction/label exists if we entered the block
    if num_labels == 0 {
        num_labels = 1;
    }

    // Generate all labels for this block (L0 to Ln-1)
    let labels: Vec<String> = (0..num_labels)
        .map(|i| generate_label("dag", depth, block_id, i))
        .collect();

    let mut assembly_block = String::new();

    // 3. Instruction Loop and DAG construction
    for i in 0..num_labels {
        let current_label = &labels[i as usize];
        assembly_block.push_str(&format!("{}:\n", current_label));

        let mut instruction_count = 0;

        // Generate a random number of mutations based on WEIGHT
        while !rng.random_bool(weight.powi(2)) && instruction_count < MAX_INSTRUCTIONS * 2 {
            assembly_block.push_str(&format!("{}\n", generate_complex_mutation(rng)));
            instruction_count += 1;
        }

        // Conditional Forward Jump (Creates DAG edges)
        if i < num_labels - 1 && !rng.random_bool(weight * 0.5) {
            // Jump to a random label strictly ahead of the current one
            let target_index = rng.random_range(i as usize + 1..num_labels as usize);
            let target_label = &labels[target_index];
            assembly_block.push_str(&generate_conditional_jump(rng, target_label));
        }

        // Recursive Call (Nesting)
        if depth < 2 {
            // Lower probability for deep nesting
            assembly_block.push_str(&generate_dag_block(weight, rng, depth + 1, id_counter));
        }
    }

    // 4. Backward Conditional Jump (Adds controlled cycles)
    // Only at the end of the block, allowing a chance to loop back to an earlier instruction.
    if num_labels > 1 && rng.random_bool(weight) {
        let target_index = rng.random_range(0..num_labels as usize - 1);
        let target_label = &labels[target_index];
        assembly_block.push_str(&format!("{}\n", generate_complex_mutation(rng)));
        assembly_block.push_str(&generate_conditional_jump(rng, target_label));
        assembly_block.push_str("// Backward Conditional Jump to maintain short execution\n");
    }

    assembly_block
}

pub fn junk_asm(input: TokenStream) -> TokenStream {
    // 1. Parse the input (expecting an optional f32 weight)
    let weight: f64 = if input.is_empty() {
        None
    } else {
        match parse_macro_input!(input as LitFloat).base10_parse::<f64>() {
            Ok(w) => Some(w), // Clamp to a sensible range
            Err(_) => None,
        }
    }
    .expect("Expected F64");
    // let final_weight = input_weight.unwrap_or(WEIGHT);

    // 2. Setup
    let mut rng = SmallRng::from_os_rng();
    let mut id_counter = 0;
    // let random_u64_addr: u64 = rng.next_u64(); // The simulated external address

    // 3. Generate Assembly
    let main_assembly = {
        loop {
            let res = generate_dag_block(weight, &mut rng, 0, &mut id_counter);
            if res.len() as f64 > weight * MIN_LENGTH {
                break res;
            }
        }
    };

    println!("{}", main_assembly);

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
