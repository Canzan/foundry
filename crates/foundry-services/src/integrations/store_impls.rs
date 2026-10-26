//! The one file of the integrations use-cases that names the store
//! (check-arch `integrations-store-capability`, DDR-26).

use super::IntegrationRecords;
use foundry_store::integrations::{
    AuditRow, CancelCandidate, ConnectionChange, ConnectionRow, ConnectionWriteOutcome,
    DeliveryHistoryRow, DeliveryIssue, DisconnectOutcome, DisconnectPreview, EmailChoiceRow,
    EmailProject, NewConnection, NewRoute, ProjectRoute, RouteChange, RouteChangeOutcome,
    RouteEdit, RouteSettings, RouteWriteOutcome, SealedConnection,
};
use foundry_store::lanes::LaneRow;
use foundry_store::teams::TeamAccessFacts;
use foundry_store::{ProjectRow, Store, StoreError};
use uuid::Uuid;

impl IntegrationRecords for Store {
    async fn is_workspace_admin(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> Result<bool, StoreError> {
        Store::is_workspace_admin(self, workspace_id, user_id).await
    }

    async fn connections(&self, workspace_id: Uuid) -> Result<Vec<ConnectionRow>, StoreError> {
        self.workspace_connections(workspace_id).await
    }

    async fn activity(&self, workspace_id: Uuid, limit: i64) -> Result<Vec<AuditRow>, StoreError> {
        self.integration_activity(workspace_id, limit).await
    }

    async fn connection_id(
        &self,
        workspace_id: Uuid,
        kind: &str,
    ) -> Result<Option<Uuid>, StoreError> {
        Store::connection_id(self, workspace_id, kind).await
    }

    async fn upsert_connection(
        &self,
        connection: &NewConnection,
    ) -> Result<ConnectionWriteOutcome, StoreError> {
        Store::upsert_connection(self, connection).await
    }

    async fn team_access(
        &self,
        workspace_id: Uuid,
        team_slug: &str,
        user_id: Uuid,
    ) -> Result<Option<TeamAccessFacts>, StoreError> {
        self.team_access_facts(workspace_id, team_slug, user_id)
            .await
    }

    async fn live_project(
        &self,
        team_id: Uuid,
        project_slug: &str,
    ) -> Result<Option<ProjectRow>, StoreError> {
        self.find_project_by_slug(team_id, project_slug).await
    }

    async fn project_routes(&self, project_id: Uuid) -> Result<Vec<ProjectRoute>, StoreError> {
        Store::project_routes(self, project_id).await
    }

    async fn sealed_connection(
        &self,
        workspace_id: Uuid,
        kind: &str,
    ) -> Result<Option<SealedConnection>, StoreError> {
        Store::sealed_connection(self, workspace_id, kind).await
    }

    async fn project_lanes(&self, project_id: Uuid) -> Result<Vec<LaneRow>, StoreError> {
        self.list_project_lanes(project_id).await
    }

    async fn latest_issue(&self, project_id: Uuid) -> Result<Option<DeliveryIssue>, StoreError> {
        Store::latest_issue(self, project_id).await
    }

    async fn create_route(&self, route: &NewRoute) -> Result<RouteWriteOutcome, StoreError> {
        Store::create_route(self, route).await
    }

    async fn route_history(
        &self,
        route_id: Uuid,
        limit: i64,
    ) -> Result<Vec<DeliveryHistoryRow>, StoreError> {
        Store::route_history(self, route_id, limit).await
    }

    async fn route_settings(
        &self,
        project_id: Uuid,
        route_id: Uuid,
    ) -> Result<Option<RouteSettings>, StoreError> {
        Store::route_settings(self, project_id, route_id).await
    }

    async fn pause_route(
        &self,
        change: &RouteChange,
        plan: impl FnOnce(&[CancelCandidate]) -> Vec<i64> + Send,
    ) -> Result<RouteChangeOutcome, StoreError> {
        Store::pause_route(self, change, plan).await
    }

    async fn resume_route(&self, change: &RouteChange) -> Result<RouteChangeOutcome, StoreError> {
        Store::resume_route(self, change).await
    }

    async fn edit_route(
        &self,
        change: &RouteChange,
        edit: &RouteEdit,
    ) -> Result<RouteChangeOutcome, StoreError> {
        Store::edit_route(self, change, edit).await
    }

    async fn delete_route(&self, change: &RouteChange) -> Result<RouteChangeOutcome, StoreError> {
        Store::delete_route(self, change).await
    }

    async fn destination_routed(
        &self,
        workspace_id: Uuid,
        kind: &str,
        destination_ref: &str,
    ) -> Result<bool, StoreError> {
        Store::destination_routed(self, workspace_id, kind, destination_ref).await
    }

    async fn disconnect_preview(
        &self,
        workspace_id: Uuid,
        kind: &str,
    ) -> Result<Option<DisconnectPreview>, StoreError> {
        Store::disconnect_preview(self, workspace_id, kind).await
    }

    async fn disconnect_connection(
        &self,
        change: &ConnectionChange,
        plan: impl FnOnce(&[CancelCandidate]) -> Vec<i64> + Send,
    ) -> Result<DisconnectOutcome, StoreError> {
        Store::disconnect_connection(self, change, plan).await
    }

    async fn email_projects(&self, user_id: Uuid) -> Result<Vec<EmailProject>, StoreError> {
        Store::email_projects(self, user_id).await
    }

    async fn email_choices(&self, user_id: Uuid) -> Result<Vec<EmailChoiceRow>, StoreError> {
        Store::email_choices(self, user_id).await
    }

    async fn save_email_choices(
        &self,
        user_id: Uuid,
        project_id: Uuid,
        choices: &[(String, String)],
    ) -> Result<bool, StoreError> {
        Store::save_email_choices(self, user_id, project_id, choices).await
    }

    async fn email_stop_project(&self, project_id: Uuid) -> Result<Option<String>, StoreError> {
        Store::email_stop_project(self, project_id).await
    }

    async fn stop_email_choice(
        &self,
        user_id: Uuid,
        project_id: Uuid,
        event: &str,
    ) -> Result<Option<String>, StoreError> {
        Store::stop_email_choice(self, user_id, project_id, event).await
    }
}
