use crate::infrai_client::{InfraiClient, InfraiError};
use serde_json::{json, Value};
use std::fmt;

#[derive(Debug, Clone)]
pub struct HandoffRequest {
    pub company_domain: String,
    pub employee_email: String,
    pub workspace: String,
    pub seller_asset: String,
    pub buyer_update: String,
    pub order_id: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct JoinDecision {
    pub workspace: String,
    pub employee_email: String,
}

#[derive(Debug)]
pub struct HandoffReceipt {
    pub user_id: String,
    pub order_id: String,
    pub workspace: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DecisionError {
    DomainNotVerified,
    EmailOutsideDomain,
}

impl fmt::Display for DecisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DomainNotVerified => write!(f, "company domain is not verified"),
            Self::EmailOutsideDomain => write!(f, "employee email does not match company domain"),
        }
    }
}

#[derive(Debug)]
pub enum HandoffError {
    Infrai(InfraiError),
    Decision(DecisionError),
    MissingField(&'static str),
}

impl fmt::Display for HandoffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Infrai(error) => write!(f, "{error}"),
            Self::Decision(error) => write!(f, "{error}"),
            Self::MissingField(field) => write!(f, "Infrai response omitted {field}"),
        }
    }
}

impl std::error::Error for HandoffError {}

impl From<InfraiError> for HandoffError {
    fn from(value: InfraiError) -> Self {
        Self::Infrai(value)
    }
}

pub fn decide_workspace_join(
    domain_verified: bool,
    company_domain: &str,
    employee_email: &str,
    workspace: &str,
) -> Result<JoinDecision, DecisionError> {
    if !domain_verified {
        return Err(DecisionError::DomainNotVerified);
    }
    let email_domain = employee_email.rsplit_once('@').map(|(_, domain)| domain);
    if email_domain != Some(company_domain) {
        return Err(DecisionError::EmailOutsideDomain);
    }
    Ok(JoinDecision {
        workspace: workspace.to_owned(),
        employee_email: employee_email.to_owned(),
    })
}

pub async fn execute_handoff(
    client: &InfraiClient,
    request: HandoffRequest,
) -> Result<HandoffReceipt, HandoffError> {
    let verification = client
        .post(
            "/v1/dns/domain/verify",
            &json!({ "domain": request.company_domain }),
        )
        .await?;
    let verified = verification
        .get("verified")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let decision = decide_workspace_join(
        verified,
        &request.company_domain,
        &request.employee_email,
        &request.workspace,
    )
    .map_err(HandoffError::Decision)?;

    // The same client sends verified seller context straight into the user directory.
    let user = client
        .post(
            "/v1/auth/user/create",
            &json!({
                "email": decision.employee_email,
                "name": decision.workspace,
                "idempotency_key": format!("workspace-join:{}:{}", request.order_id, request.employee_email),
                "metadata": {
                    "workspace": request.workspace,
                    "company_domain": request.company_domain,
                    "seller_asset": request.seller_asset,
                    "buyer_update": request.buyer_update,
                    "order_id": request.order_id
                }
            }),
        )
        .await?;

    Ok(HandoffReceipt {
        user_id: required_string(&user, "id")?,
        order_id: request.order_id,
        workspace: request.workspace,
    })
}

fn required_string(value: &Value, field: &'static str) -> Result<String, HandoffError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or(HandoffError::MissingField(field))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_only_matching_employee_of_verified_company() {
        let joined = decide_workspace_join(
            true,
            "northwind.example",
            "ops@northwind.example",
            "northwind-sellers",
        );
        assert_eq!(
            joined,
            Ok(JoinDecision {
                workspace: "northwind-sellers".into(),
                employee_email: "ops@northwind.example".into(),
            })
        );
        assert_eq!(
            decide_workspace_join(
                false,
                "northwind.example",
                "ops@northwind.example",
                "northwind-sellers"
            ),
            Err(DecisionError::DomainNotVerified)
        );
        assert_eq!(
            decide_workspace_join(
                true,
                "northwind.example",
                "ops@other.example",
                "northwind-sellers"
            ),
            Err(DecisionError::EmailOutsideDomain)
        );
    }
}
