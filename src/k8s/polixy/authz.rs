use super::super::labels;
use crate::FromResource;
use kube::{api::Resource, CustomResource};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Name(String);

/// Authorizes clients to connect to a Server.
#[kube(
    group = "polixy.olix0r.net",
    version = "v1",
    kind = "Authorization",
    namespaced
)]
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizationSpec {
    pub server: Server,
    pub client: Client,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct Server {
    pub name: Option<super::server::Name>,
    pub selector: Option<labels::Selector>,
}

/// Describes an authenticated client.
///
/// Exactly one of `identities` and `service_accounts` should be set.
#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Client {
    pub cidrs: Option<Vec<String>>,

    pub unauthenticated: Option<bool>,

    /// Indicates a Linkerd identity that is authorized to access a server.
    pub identities: Option<Vec<String>>,

    /// Identifies a `ServiceAccount` authorized to access a server.
    pub service_accounts: Option<Vec<ServiceAccountRef>>,
}

/// References a Kubernetes `ServiceAccount` instance.
///
/// If no namespace is specified, the `Authorization`'s namespace is used.
#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct ServiceAccountRef {
    pub namespace: Option<String>,
    pub name: String,
    // TODO pub selector: labels::Selector,
}

// === Name ===

impl FromResource<Authorization> for Name {
    fn from_resource(s: &Authorization) -> Self {
        Self(s.name())
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
