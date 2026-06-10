#![cfg(feature = "spawn")]

use hearsay::{
    App, AppStatus, Broker, RestartPolicy, app_status, drain_output, spawn_app, start_broker,
    stop_app,
};
use std::time::Duration;

async fn wait_for_exit(broker: &Broker, name: &str) -> AppStatus {
    loop {
        let status = app_status(broker, name).await;
        if !matches!(status, AppStatus::Running) {
            return status;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn spawned_app_reports_output_and_exit() -> hearsay::Result<()> {
    let broker = start_broker("127.0.0.1:9941").await?;
    spawn_app(
        &broker,
        App {
            name: "version".to_string(),
            path: "rustc".to_string(),
            args: "--version".to_string(),
            ..Default::default()
        },
    )
    .await?;

    let status =
        tokio::time::timeout(Duration::from_secs(30), wait_for_exit(&broker, "version")).await?;
    assert_eq!(status, AppStatus::ExitedSuccessfully);

    tokio::time::sleep(Duration::from_millis(250)).await;
    let lines = drain_output(&broker).await;
    assert!(lines.iter().any(|line| line.line.contains("rustc")));
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn spawned_app_receives_broker_address() -> hearsay::Result<()> {
    let broker_address = "127.0.0.1:9942";
    let broker = start_broker(broker_address).await?;

    #[cfg(target_os = "windows")]
    let app = App {
        name: "environment".to_string(),
        path: "cmd".to_string(),
        args: "/c set".to_string(),
        ..Default::default()
    };
    #[cfg(not(target_os = "windows"))]
    let app = App {
        name: "environment".to_string(),
        path: "env".to_string(),
        ..Default::default()
    };
    spawn_app(&broker, app).await?;

    let status = tokio::time::timeout(
        Duration::from_secs(30),
        wait_for_exit(&broker, "environment"),
    )
    .await?;
    assert_eq!(status, AppStatus::ExitedSuccessfully);

    tokio::time::sleep(Duration::from_millis(250)).await;
    let lines = drain_output(&broker).await;
    let expected = format!("HEARSAY_BROKER={broker_address}");
    assert!(lines.iter().any(|line| line.line.contains(&expected)));
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn always_restart_policy_relaunches_exited_apps() -> hearsay::Result<()> {
    let broker = start_broker("127.0.0.1:9943").await?;
    spawn_app(
        &broker,
        App {
            name: "repeater".to_string(),
            path: "rustc".to_string(),
            args: "--version".to_string(),
            restart_policy: RestartPolicy::Always,
            ..Default::default()
        },
    )
    .await?;

    tokio::time::sleep(Duration::from_secs(3)).await;
    let lines = drain_output(&broker).await;
    let launches = lines
        .iter()
        .filter(|line| line.line.contains("rustc"))
        .count();
    assert!(
        launches >= 2,
        "expected at least two launches, saw {launches}"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn stopped_apps_stay_stopped() -> hearsay::Result<()> {
    let broker = start_broker("127.0.0.1:9944").await?;

    #[cfg(target_os = "windows")]
    let app = App {
        name: "long_running".to_string(),
        path: "cmd".to_string(),
        args: "/c ping -n 60 127.0.0.1".to_string(),
        ..Default::default()
    };
    #[cfg(not(target_os = "windows"))]
    let app = App {
        name: "long_running".to_string(),
        path: "sleep".to_string(),
        args: "60".to_string(),
        ..Default::default()
    };
    spawn_app(&broker, app).await?;

    assert_eq!(
        app_status(&broker, "long_running").await,
        AppStatus::Running
    );
    stop_app(&broker, "long_running").await?;
    assert_eq!(
        app_status(&broker, "long_running").await,
        AppStatus::Stopped
    );

    tokio::time::sleep(Duration::from_secs(1)).await;
    assert_eq!(
        app_status(&broker, "long_running").await,
        AppStatus::Stopped
    );
    Ok(())
}
