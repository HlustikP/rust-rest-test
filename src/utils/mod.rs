use std::{env, path::PathBuf};

pub fn get_cwd() -> PathBuf {
    return env::current_dir().unwrap();
}

// Get the digit count of a number to a given base
pub fn get_num_digits<T: std::ops::Div<T, Output = T> + std::cmp::PartialOrd + std::marker::Copy>
    (number: T, base: T) -> u8 {

    let iterations = 0;
    let mut test_number = number;

    while test_number >= base {
        test_number = test_number / base;
    }

    return iterations;
}

// Prints a string and appends a newline if a given condition evaluates to true
#[macro_export]
macro_rules! printlnif {
    ($condition: expr, $($x:tt)*) => { 
        if $condition { 
            println!($($x)*);
        }
    }
}

// Prints a string if a given condition evaluates to true
#[macro_export]
macro_rules! printif {
    ($condition: expr, $($x:tt)*) => { 
        if $condition { 
            println!($($x)*);
        }
    }
}
