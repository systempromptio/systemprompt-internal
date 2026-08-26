//! A wiremock Odoo: the whole integration surface is one POST `/jsonrpc`.
//!
//! `common.authenticate` proves a credential and returns a uid; `execute_kw`
//! reads `res.users.groups_id` and resolves them through `ir.model.data`.
//! The mock dispatches on the JSON-RPC body and keeps the group answer in
//! shared state, so a test can flip a user's groups (or break the lookup)
//! between logins and watch the platform roles follow.

use std::sync::{Arc, Mutex};

use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate, matchers};

pub const UID: i64 = 7;
pub const GOOD_CREDENTIAL: &str = "e2e-odoo-credential";

#[derive(Debug, Clone)]
pub enum Groups {
    XmlIds(Vec<(&'static str, &'static str)>),
    LookupFails,
}

pub struct OdooMock {
    pub server: MockServer,
    groups: Arc<Mutex<Groups>>,
}

struct OdooRpc {
    groups: Arc<Mutex<Groups>>,
    notes: Arc<Mutex<Vec<serde_json::Value>>>,
}

fn rpc_result(value: serde_json::Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": value,
    }))
}

fn rpc_fault(message: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "error": { "code": 200, "message": message },
    }))
}

impl Respond for OdooRpc {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: serde_json::Value = match serde_json::from_slice(&request.body) {
            Ok(body) => body,
            Err(_) => return rpc_fault("unparseable request"),
        };
        let params = &body["params"];
        match (params["service"].as_str(), params["method"].as_str()) {
            (Some("common"), Some("authenticate")) => {
                let credential = params["args"][2].as_str().unwrap_or_default();
                if credential == GOOD_CREDENTIAL {
                    rpc_result(serde_json::json!(UID))
                } else {
                    rpc_result(serde_json::json!(false))
                }
            },
            (Some("object"), Some("execute_kw")) => self.execute_kw(params),
            _ => rpc_fault("unknown service"),
        }
    }
}

impl OdooRpc {
    fn execute_kw(&self, params: &serde_json::Value) -> ResponseTemplate {
        let model = params["args"][3].as_str().unwrap_or_default();
        let method = params["args"][4].as_str().unwrap_or_default();
        if method == "message_post" {
            return self.message_post(model, params);
        }
        match model {
            "crm.lead" => rpc_result(serde_json::json!([{
                "id": 1,
                "name": "E2E Table Lead",
                "partner_name": "Acme",
                "email_from": "buyer@acme.test",
                "phone": false,
                "stage_id": [1, "New"],
                "user_id": [UID, "E2E Person"],
                "expected_revenue": 1250.5,
                "probability": 40.0,
                "create_date": "2026-08-26 09:00:00",
            }])),
            "mail.message" => {
                let notes = self.notes.lock().expect("notes state").clone();
                rpc_result(serde_json::Value::Array(notes))
            },
            "res.users" => rpc_result(serde_json::json!([
                { "id": UID, "groups_id": [1, 2, 3] }
            ])),
            "ir.model.data" => {
                let groups = self.groups.lock().expect("groups state").clone();
                match groups {
                    Groups::XmlIds(ids) => rpc_result(serde_json::Value::Array(
                        ids.iter()
                            .map(|(module, name)| {
                                serde_json::json!({ "module": module, "name": name })
                            })
                            .collect(),
                    )),
                    Groups::LookupFails => rpc_fault("access denied on ir.model.data"),
                }
            },
            _ => rpc_fault("unknown model"),
        }
    }

    // Record the posted note the way Odoo's chatter would hand it back to a
    // later `mail.message` search: anchored to its record, body intact.
    fn message_post(&self, model: &str, params: &serde_json::Value) -> ResponseTemplate {
        let res_id = params["args"][5][0][0].as_i64().unwrap_or_default();
        let body = params["args"][6]["body"].as_str().unwrap_or_default();
        let mut notes = self.notes.lock().expect("notes state");
        let id = 100 + i64::try_from(notes.len()).unwrap_or_default();
        notes.push(serde_json::json!({
            "id": id,
            "model": model,
            "res_id": res_id,
            "record_name": "E2E Lead",
            "author_id": [UID, "E2E Person"],
            "date": "2026-08-26 09:00:00",
            "message_type": "comment",
            "body": format!("<p>{body}</p>"),
        }));
        rpc_result(serde_json::json!(id))
    }
}

impl OdooMock {
    pub async fn start() -> Self {
        let server = MockServer::start().await;
        let groups = Arc::new(Mutex::new(Groups::XmlIds(vec![("base", "group_user")])));
        let notes = Arc::new(Mutex::new(Vec::new()));
        Mock::given(matchers::method("POST"))
            .and(matchers::path("/jsonrpc"))
            .respond_with(OdooRpc {
                groups: Arc::clone(&groups),
                notes: Arc::clone(&notes),
            })
            .mount(&server)
            .await;
        Self { server, groups }
    }

    pub fn url(&self) -> String {
        self.server.uri()
    }

    pub fn set_groups(&self, groups: Groups) {
        *self.groups.lock().expect("groups state") = groups;
    }
}
