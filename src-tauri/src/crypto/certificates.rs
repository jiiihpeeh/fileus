use rcgen::{Certificate, CertificateParams, DistinguishedName, IsCa, KeyPair};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use time::OffsetDateTime;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TlsCertificates {
    pub ca_cert: String,
    pub ca_key: String,
    pub domain: String,
    pub domain_cert: String,
    pub domain_key: String,
}

pub fn generate_ca() -> Result<(Certificate, String, String), String> {
    let mut params = CertificateParams::default();
    params.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "Local Rust CA");
    params.not_before = OffsetDateTime::now_utc();
    params.not_after = params.not_before + time::Duration::days(365);

    let key_pair = KeyPair::generate().map_err(|e| e.to_string())?;
    let ca_cert = params.self_signed(&key_pair).map_err(|e| e.to_string())?;
    let ca_pem = ca_cert.pem();
    let ca_key = key_pair.serialize_pem();

    Ok((ca_cert, ca_pem, ca_key))
}

pub fn generate_domain_cert(
    ca_cert: &Certificate,
    ca_key: &KeyPair,
    domain: &str,
) -> Result<(String, String), String> {
    let mut params = CertificateParams::new(vec![domain.to_string()]).map_err(|e| e.to_string())?;
    params.distinguished_name = DistinguishedName::new();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, domain);
    params.not_before = OffsetDateTime::now_utc();
    params.not_after = params.not_before + time::Duration::days(365);

    let key_pair = KeyPair::generate().map_err(|e| e.to_string())?;
    let cert = params
        .signed_by(&key_pair, ca_cert, ca_key)
        .map_err(|e| e.to_string())?;
    let pem = cert.pem();
    let key = key_pair.serialize_pem();

    Ok((pem, key))
}

#[tauri::command]
pub fn generate_local_certs(domain: Option<String>) -> Result<TlsCertificates, String> {
    let domain = domain.unwrap_or_else(|| "localhost".to_string());
    let (ca_cert, ca_pem, ca_key) = generate_ca()?;
    let key_pair = KeyPair::from_pem(&ca_key).map_err(|e| e.to_string())?;
    let (domain_cert, domain_key) = generate_domain_cert(&ca_cert, &key_pair, &domain)?;

    let mut cert_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    cert_dir.push("certs");
    std::fs::create_dir_all(&cert_dir).map_err(|e| format!("Failed to create certs dir: {}", e))?;

    std::fs::write(cert_dir.join("ca_cert.pem"), &ca_pem)
        .map_err(|e| format!("Failed to write ca_cert.pem: {}", e))?;
    std::fs::write(cert_dir.join("ca_key.pem"), &ca_key)
        .map_err(|e| format!("Failed to write ca_key.pem: {}", e))?;
    std::fs::write(cert_dir.join(format!("{}.pem", domain)), &domain_cert)
        .map_err(|e| format!("Failed to write domain cert: {}", e))?;
    std::fs::write(cert_dir.join(format!("{}-key.pem", domain)), &domain_key)
        .map_err(|e| format!("Failed to write domain key: {}", e))?;

    Ok(TlsCertificates {
        ca_cert: ca_pem,
        ca_key,
        domain,
        domain_cert,
        domain_key,
    })
}

#[tauri::command]
pub fn generate_tls_certificates(domain: Option<String>) -> Result<TlsCertificates, String> {
    let domain = domain.unwrap_or_else(|| "localhost".to_string());
    let (ca_cert, ca_pem, ca_key) = generate_ca()?;
    let key_pair = KeyPair::from_pem(&ca_key).map_err(|e| e.to_string())?;
    let (domain_cert, domain_key) = generate_domain_cert(&ca_cert, &key_pair, &domain)?;
    Ok(TlsCertificates {
        ca_cert: ca_pem,
        ca_key,
        domain,
        domain_cert,
        domain_key,
    })
}
