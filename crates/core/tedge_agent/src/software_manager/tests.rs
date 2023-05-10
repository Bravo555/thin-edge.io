use crate::software_manager::actor::SoftwareRequest;
use crate::software_manager::actor::SoftwareResponse;
use crate::software_manager::builder::SoftwareManagerBuilder;
use crate::software_manager::config::SoftwareManagerConfig;
use std::time::Duration;
use tedge_actors::test_helpers::MessageReceiverExt;
use tedge_actors::test_helpers::TimedMessageBox;
use tedge_actors::Actor;
use tedge_actors::Builder;
use tedge_actors::DynError;
use tedge_actors::MessageReceiver;
use tedge_actors::Sender;
use tedge_actors::ServiceConsumer;
use tedge_actors::SimpleMessageBox;
use tedge_actors::SimpleMessageBoxBuilder;
use tedge_api::OperationStatus;
use tedge_api::SoftwareListRequest;
use tedge_api::SoftwareListResponse;
use tedge_api::SoftwareRequestResponse;
use tedge_api::SoftwareUpdateResponse;
use tedge_config::TEdgeConfigLocation;
use tedge_test_utils::fs::TempTedgeDir;

const TEST_TIMEOUT_MS: Duration = Duration::from_millis(5000);

#[tokio::test]
async fn test_pending_software_update_operation() -> Result<(), DynError> {
    let temp_dir = TempTedgeDir::new();
    let content = "operation_id = \'1234\'\noperation = \"update\"";
    temp_dir
        .dir(".agent")
        .file("software-current-operation")
        .with_raw_content(content);

    let mut converter_box = spawn_software_manager(&temp_dir).await?;

    let software_request_response = SoftwareRequestResponse::new("1234", OperationStatus::Failed);
    converter_box
        .assert_received([SoftwareUpdateResponse {
            response: software_request_response,
        }])
        .await;

    Ok(())
}

// #[tokio::test]
// async fn test_new_software_list_operation() -> Result<(), DynError> {
//     let temp_dir = TempTedgeDir::new();
//     temp_dir.dir(".agent").file("current-operation");
//
//     // Spawn restart manager
//     let mut converter_box = spawn_software_manager(&temp_dir).await?;
//
//     // Simulate RestartOperationRe
//     converter_box
//         .send(SoftwareUpdateRequest {
//             id: "random".to_string(),
//             update_list: vec![]
//         })
//         .await?;
//
//     let status = converter_box.recv().await.unwrap().status;
//     assert_eq!(status, OperationStatus::Executing);
//
//     Ok(())
// }

#[tokio::test]
async fn test_pending_software_list_operation() -> Result<(), DynError> {
    let temp_dir = TempTedgeDir::new();
    let content = "operation_id = \'1234\'\noperation = \"list\"";
    temp_dir
        .dir(".agent")
        .file("software-current-operation")
        .with_raw_content(content);

    let mut converter_box = spawn_software_manager(&temp_dir).await?;

    let software_request_response = SoftwareRequestResponse::new("1234", OperationStatus::Failed);
    converter_box
        .assert_received([SoftwareListResponse {
            response: software_request_response,
        }])
        .await;

    Ok(())
}

#[tokio::test]
async fn test_new_software_list_operation() -> Result<(), DynError> {
    let temp_dir = TempTedgeDir::new();
    temp_dir.dir(".agent");

    // Spawn restart manager
    let mut converter_box = spawn_software_manager(&temp_dir).await?;

    // Simulate RestartOperationRe
    converter_box
        .send(
            SoftwareListRequest {
                id: "random".to_string(),
            }
            .into(),
        )
        .await?;

    let response = converter_box.recv().await.unwrap();
    match response {
        SoftwareResponse::SoftwareListResponse(res) => {
            assert_eq!(res.response.status, OperationStatus::Executing);
        }
        SoftwareResponse::SoftwareUpdateResponse(_) => {
            assert!(false, "Received SoftwareUpdateResponse")
        }
    }

    Ok(())
}

async fn spawn_software_manager(
    tmp_dir: &TempTedgeDir,
) -> Result<TimedMessageBox<SimpleMessageBox<SoftwareResponse, SoftwareRequest>>, DynError> {
    let mut converter_builder: SimpleMessageBoxBuilder<SoftwareResponse, SoftwareRequest> =
        SimpleMessageBoxBuilder::new("Converter", 5);

    let config = SoftwareManagerConfig::new(
        &tmp_dir.utf8_path_buf(),
        &tmp_dir.utf8_path_buf(),
        &tmp_dir.utf8_path_buf(),
        &tmp_dir.utf8_path_buf(),
        None,
        &TEdgeConfigLocation::from_custom_root(tmp_dir.utf8_path_buf()),
    );

    let mut software_actor_builder = SoftwareManagerBuilder::new(config);
    converter_builder.set_connection(&mut software_actor_builder);

    let converter_box = converter_builder.build().with_timeout(TEST_TIMEOUT_MS);

    let mut software_actor = software_actor_builder.build();
    tokio::spawn(async move { software_actor.run().await });

    Ok(converter_box)
}
