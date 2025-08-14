use std::collections::HashMap;

fn main() {
    // Create a simple test to understand where the instructions go
    println!("Testing IR generation for while loop");
    
    // The expected structure should be:
    // entry block: 
    //   - initialize count
    //   - jump to loop_start
    // loop_start block:
    //   - check condition
    //   - jump if true to body, else to end
    // loop_body block:
    //   - increment count
    //   - jump back to start
    // loop_end block:
    //   - return value
    
    println!("A proper while loop CFG needs multiple blocks with explicit jumps");
}
