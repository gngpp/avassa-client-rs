/// CA Certificate helpers
pub mod ca {
    use crate::{Client, Result};
    use serde::{Deserialize, Serialize};
    /// Represents a Strongbox CA certificate
    #[derive(Debug, Deserialize)]
    pub struct Certificate {
        /// Name of the CA
        pub name: String,
        /// Key type
        #[serde(rename = "cert-key-type")]
        pub cert_key_type: String,
        /// Certificate in PEM format
        #[serde(rename = "ca-cert")]
        pub ca_cert_pem: String,
    }

    /// Queries Stronbox for available CA certificates
    pub async fn certificates(client: &Client) -> Result<Vec<Certificate>> {
        let cas = client.get_json("/v1/config/strongbox/tls/ca", None).await?;
        Ok(cas)
    }

    /// Returns all roles for a CA certificate
    pub async fn certificate_roles(client: &Client, ca_name: &str) -> Result<Vec<Role>> {
        let roles = client
            .get_json(&format!("/v1/config/strongbox/tls/ca/{ca_name}/role"), None)
            .await?;
        Ok(roles)
    }

    /// Represents a Strongbox CA certificate
    #[derive(Debug, Deserialize, Serialize)]
    pub struct Role {
        /// Name of the Role
        pub name: String,
        /// Time to live for the role
        pub ttl: std::time::Duration,
    }

    /// Create or update a CA role
    pub async fn create_certificate_role(
        client: &Client,
        ca_name: &str,
        role: &Role,
    ) -> Result<()> {
        let _put_result = client
            .put_json(
                &format!("/v1/config/strongbox/tls/ca/{ca_name}/role/{}", role.name),
                &serde_json::to_value(role)?,
            )
            .await?;
        Ok(())
    }
}

/// Function for generating server certificates
pub mod server_certificate {
    use crate::{Client, Result};
    use serde::Deserialize;

    /// Representation of a server certificate
    #[derive(Debug, Deserialize)]
    pub struct Certificate {
        /// Certificate in PEM format
        pub cert: String,
        /// Private key
        #[serde(rename = "private-key")]
        pub private_key: String,
        /// Serial number
        pub serial: String,
        /// Expiry date
        pub expires: chrono::DateTime<chrono::offset::FixedOffset>,
    }

    /// Generate a server certificate for an application
    pub async fn create_certificate(
        client: &Client,
        ca_name: &str,
        role_name: &str,
        ttl: std::time::Duration,
        host: &str,
        alt_name: &str,
    ) -> Result<Certificate> {
        let req = serde_json::json!({
            "ttl": ttl,
            "host": host,
            "cert-type": "server",
            "alt-name": alt_name,
        });

        let cert = client
            .post_json(
                &format!("/v1/state/strongbox/tls/ca/{ca_name}/role/{role_name}/issue-cert",),
                &req,
            )
            .await?;

        let cert = serde_json::from_value(cert)?;

        Ok(cert)
    }
}
