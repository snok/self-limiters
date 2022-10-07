use std::time::Duration;

use crate::utils::{get_script, now_millis, SLResult};

#[test]
fn test_get_script() -> SLResult<()> {
    get_script("scripts/schedule.lua")?;
    get_script("scripts/create_semaphore.lua")?;
    get_script("scripts/release_semaphore.lua")?;
    Ok(())
}

#[tokio::test]
async fn test_now_millis() -> SLResult<()> {
    let now = now_millis()?;
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(now + 30 <= now_millis()?);
    assert!(now + 33 >= now_millis()?);
    Ok(())
}
