use cond_sync::{CondSync, Other};
use std::{thread, time::Duration};
const NO_OF_THREADS: usize = 5;

#[test]
fn test() {
    let cond_sync = CondSync::new(0_usize); // <- use a plain usize as condition state

    for i in 0..NO_OF_THREADS {
        let cond_sync_t = cond_sync.clone();
        thread::spawn(move || {
            println!("Thread {i}: initializing ...");
            thread::sleep(Duration::from_millis(47));
            cond_sync_t
                .modify_and_notify(|v| *v += 1, Other::One)
                .unwrap(); // <- modify the state

            thread::sleep(Duration::from_millis(1)); // just to produce a yield
            println!("Thread {i}: work on phase 1");
        });
    }
    assert!(cond_sync
        .wait_until_or_timeout(|v| *v == NO_OF_THREADS, Duration::from_millis(800))
        .unwrap()
        .is_condition());

    println!("Main: All threads initialized");
    thread::sleep(Duration::from_millis(100)); // just to let the threads finish (better use join)}
}
