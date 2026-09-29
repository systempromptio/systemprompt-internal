//! `repositories::projects::crud` — project lifecycle is deliberately
//! independent from group lifecycle: there is no protected system project,
//! and deleting completed work removes its attribution membership.

use systemprompt_web_admin::repositories::projects::crud::{
    delete_project, find_project, insert_project, list_projects, update_project,
};
use systemprompt_web_admin::types::projects::{CreateProjectRequest, UpdateProjectRequest};

use crate::fixtures::unique_project;
use crate::tempdb::TempDb;

fn create(id: systemprompt_web_shared::ProjectId) -> CreateProjectRequest {
    CreateProjectRequest {
        id,
        name: "Migration programme".to_owned(),
        description: Some("Tracks work during the tenant migration".to_owned()),
    }
}

#[tokio::test]
async fn a_created_project_is_readable_and_keeps_unnamed_fields_on_update() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let request = create(unique_project("project"));
    insert_project(&db.pool, &request, "dashboard")
        .await
        .expect("create project");

    let updated = update_project(
        &db.pool,
        &request.id,
        &UpdateProjectRequest {
            name: Some("Renamed programme".to_owned()),
            description: None,
        },
    )
    .await
    .expect("rename project");
    assert_eq!(updated.name, "Renamed programme");
    assert_eq!(
        updated.description, request.description,
        "an omitted description means retain it, rather than erase it"
    );
    assert_eq!(
        find_project(&db.pool, &request.id)
            .await
            .expect("read project")
            .expect("created row")
            .source,
        "dashboard"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn duplicate_and_missing_project_mutations_return_caller_errors() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let request = create(unique_project("project"));
    insert_project(&db.pool, &request, "dashboard")
        .await
        .expect("initial creation");

    assert_eq!(
        insert_project(&db.pool, &request, "dashboard")
            .await
            .expect_err("same id is refused")
            .status()
            .as_u16(),
        409
    );
    let absent = unique_project("missing");
    assert_eq!(
        update_project(
            &db.pool,
            &absent,
            &UpdateProjectRequest {
                name: Some("Nobody".to_owned()),
                description: None,
            },
        )
        .await
        .expect_err("missing update")
        .status()
        .as_u16(),
        404
    );
    assert_eq!(
        delete_project(&db.pool, &absent)
            .await
            .expect_err("missing delete")
            .status()
            .as_u16(),
        404
    );
    db.cleanup().await;
}

#[tokio::test]
async fn deleting_a_project_removes_it_from_the_listing() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let request = create(unique_project("project"));
    insert_project(&db.pool, &request, "dashboard")
        .await
        .expect("create project");

    delete_project(&db.pool, &request.id)
        .await
        .expect("delete project");
    assert!(
        find_project(&db.pool, &request.id)
            .await
            .expect("read after delete")
            .is_none()
    );
    assert!(
        !list_projects(&db.pool)
            .await
            .expect("list projects")
            .iter()
            .any(|project| project.id == request.id)
    );
    db.cleanup().await;
}
