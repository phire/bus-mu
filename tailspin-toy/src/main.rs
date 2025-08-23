#![allow(incomplete_features)]
#![feature(explicit_tail_calls)]

use tailspin::tailspin;

struct State {
   regs: [u64; 32],
}

tailspin! {
    @state(State)

    @op( Add(a: u64, b: u64) -> u64 {a.wrapping_add(b)})
    @op( Exit() -> @exit {
        println!("Exiting...");

        ExitCode(0)
    })
}

fn linked_list_rust(mem: &mut [u64], count: usize) {
    // Start with a terminal node at 2
    mem[2 + 0] = 0; // node+0 is the number
    mem[2 + 1] = 0; // node+1 is next pointer

    mem[1] = 0xff; // Pointer to the lowest node
    mem[0] = 2; // Mem[0] is the bump allocator pointer, points to last allocated node

    for _ in 0..count {
        let mut prev = 2usize;
        let mut next = mem[prev + 1] as usize;

        // get a random number
        let x = rand::random::<u64>();

        // find where it belongs
        while next != 0 && mem[next + 0] < x {
            prev = next;
            next = mem[next + 1] as usize;
        }

        // allocate a node with bump allocator
        let node = {
            mem[0] += 2;
            mem[0] as usize
        };

        // fill it new node and link it
        mem[node + 0] = x;
        mem[node + 1] = next as u64;
        mem[prev + 1] = node as u64;
    }
}

fn linked_list_tailspin() {

}

fn main() {
    println!("Tailspin Toy");
    let count = 30;

    let mut mem = vec![0u64; 0x100000];
    linked_list_rust(mem.as_mut_slice(), count);

    // for i in 0..(count + 2) {
    //     println!("mem[{:x}] = {:x}, {:x}", i * 2, mem[i * 2], mem[i * 2 + 1])
    // }

    check_list(mem.as_slice(), count);
}

fn check_list(mem: &[u64], mut count: usize) -> bool {
    let prev = 0;
    let mut ptr = 2usize;
    loop {
        if ptr >= mem.len() {
            println!("Pointer out of bounds: {:04x}", ptr);
            return false;
        }

        //println!("mem[{:04x}] = {:x}", ptr, mem[ptr]);
        let value = mem[ptr];
        if prev > value {
            println!("List is not sorted: prev = {:x}, value = {:x}", prev, value);
            return false;
        }

        ptr = mem[ptr + 1] as usize;

        if ptr == 0 {
            break;
        }

        if count == 0 {
            println!("List too long");
            return false;
        }
        count -= 1;
    }

    if count != 0 {
        println!("List too short, expected {} elements, got more", count);
        return false;
    }

    return true;
}