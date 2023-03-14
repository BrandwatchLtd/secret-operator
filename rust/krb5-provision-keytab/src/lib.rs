//! API wrapper for accessing

use std::{
    path::{Path, PathBuf},
    process::Stdio,
};

use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};
use tokio::{io::AsyncWriteExt, process::Command};

#[derive(Serialize, Deserialize)]
pub struct Request {
    pub admin_keytab_path: PathBuf,
    pub admin_principal_name: String,
    pub pod_keytab_path: PathBuf,
    pub principals: Vec<PrincipalRequest>,
}
#[derive(Serialize, Deserialize)]
pub struct PrincipalRequest {
    pub name: String,
}

#[derive(Serialize, Deserialize)]
pub struct Response {}

#[derive(Snafu, Debug)]
pub enum Error {
    #[snafu(display("failed to serialize request"))]
    SerializeRequest { source: serde_json::Error },
    #[snafu(display("failed to deserialize response"))]
    DeserializeResponse { source: serde_json::Error },
    #[snafu(display("failed to start provisioner"))]
    SpawnProvisioner { source: std::io::Error },
    #[snafu(display("error waiting for provisioner to exit"))]
    WaitProvisioner { source: std::io::Error },
    #[snafu(display("failed to provision keytab: {msg}"))]
    RunProvisioner { msg: String },
    #[snafu(display("failed to write request"))]
    WriteRequest { source: std::io::Error },
}

/// Provisions a Kerberos Keytab based on the [`Request`].
///
/// This function assumes that the binary produced by this crate is on the `$PATH`, and will fail otherwise.
pub async fn provision_keytab(krb5_config_path: &Path, req: &Request) -> Result<Response, Error> {
    let req_str = serde_json::to_vec(&req).context(SerializeRequestSnafu)?;
    let mut child = Command::new("stackable-krb5-provision-keytab")
        .kill_on_drop(true)
        .env("KRB5_CONFIG", krb5_config_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context(SpawnProvisionerSnafu)?;
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(&req_str).await.context(WriteRequestSnafu)?;
    stdin.flush().await.context(WriteRequestSnafu)?;
    drop(stdin);
    let output = child
        .wait_with_output()
        .await
        .context(WaitProvisionerSnafu)?;
    serde_json::from_slice::<Result<Response, String>>(&output.stdout)
        .context(DeserializeResponseSnafu)?
        .map_err(|msg| Error::RunProvisioner { msg })
}
