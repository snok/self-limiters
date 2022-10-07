use std::thread;

use redis::Client;

use self_limiters_core::semaphore::ThreadState as SemaphoreThreadState;
use self_limiters_core::token_bucket::ThreadState as TokenBucketThreadState;
use self_limiters_core::utils::{get_script, SLResult};

use crate::errors::PSLError;
use crate::utils::{send_shared_state, validate_redis_url};

#[test]
fn test_send_and_receive_via_channel_semaphore_threaded() -> Result<(), PSLError> {
    let client = Client::open("redis://127.0.0.1:6389").unwrap();
    let name = String::from("test");
    let capacity = 1;
    let max_sleep = 0.0;

    // Send and receive w/o thread
    let receiver = send_shared_state(SemaphoreThreadState {
        client,
        name: name.to_owned(),
        capacity: capacity.to_owned(),
        max_sleep: max_sleep.to_owned(),
    })?;
    let copied_ts = thread::spawn(move || receiver.recv().unwrap())
        .join()
        .unwrap();
    assert_eq!(copied_ts.name, name);
    assert_eq!(copied_ts.capacity, capacity);
    Ok(())
}

#[test]
fn test_send_and_receive_via_channel_token_bucket_threaded() -> Result<(), PSLError> {
    let client = Client::open("redis://127.0.0.1:6389").unwrap();
    let name = String::from("test");
    let capacity = 1;
    let max_sleep = 10.0;
    let frequency = 0.1;
    let amount = 1;

    // Send and receive w/o thread
    let receiver = send_shared_state(TokenBucketThreadState {
        client,
        name: name.to_owned(),
        capacity: capacity.to_owned(),
        max_sleep: max_sleep.to_owned(),
        frequency: frequency.to_owned(),
        amount: amount.to_owned(),
    })
    .unwrap();
    thread::spawn(move || {
        let copied_ts = receiver.recv().unwrap();
        assert_eq!(copied_ts.name, name);
        assert_eq!(copied_ts.capacity, capacity);
        assert_eq!(copied_ts.max_sleep, max_sleep);
        assert_eq!(copied_ts.frequency, frequency);
        assert_eq!(copied_ts.amount, amount);
    });
    Ok(())
}

#[test]
fn test_validate_redis_urls() {
    // Make sure these normal URLs pass parsing
    for good_url in &[
        "redis://127.0.0.1",
        "redis://username:@127.0.0.1",
        "redis://username:password@127.0.0.1",
        "redis://:password@127.0.0.1",
        "redis+unix:///127.0.0.1",
        "unix:///127.0.0.1",
    ] {
        for port_postfix in &[":6379", ":1234", ""] {
            validate_redis_url(Some(&format!("{}{}", good_url, port_postfix))).unwrap();
        }
    }

    // None is also allowed, and we will try to connect to the default address
    validate_redis_url(None).unwrap();

    // Make sure these bad URLs fail
    for bad_url in &["", "1", "127.0.0.1:6379", "test://127.0.0.1:6379"] {
        if let Ok(_) = validate_redis_url(Some(bad_url)) {
            panic!("Should fail")
        }
    }
}

#[test]
fn test_get_script() -> SLResult<()> {
    get_script("scripts/schedule.lua")?;
    get_script("scripts/create_semaphore.lua")?;
    get_script("scripts/release_semaphore.lua")?;
    Ok(())
}
