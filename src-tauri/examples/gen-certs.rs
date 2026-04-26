use rcgen::{Certificate, CertificateParams, DistinguishedName, IsCa, KeyPair};
use time::OffsetDateTime;
use std::fs;

fn generate_ca() -> Result<(Certificate, String, String), Box<dyn std::error::Error>> {
    let mut params = CertificateParams::default();
    params.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    params.distinguished_name.push(rcgen::DnType::CommonName, "Local Rust CA");
    params.not_before = OffsetDateTime::now_utc();
    params.not_after = params.not_before + time::Duration::days(365);
    let key_pair = KeyPair::generate()?;
    let ca_cert = params.self_signed(&key_pair)?;
    let ca_pem = ca_cert.pem();
    let ca_key = key_pair.serialize_pem();
    Ok((ca_cert, ca_pem, ca_key))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (ca_cert, ca_pem, ca_key) = generate_ca()?;
    fs::create_dir_all("certs")?;
    fs::write("certs/ca_cert.pem", &ca_pem)?;
    fs::write("certs/ca_key.pem", &ca_key)?;
    let mut params = CertificateParams::new(vec!["localhost".to_string()])?;
    params.distinguished_name = DistinguishedName::new();
    params.distinguished_name.push(rcgen::DnType::CommonName, "localhost");
    params.not_before = OffsetDateTime::now_utc();
    params.not_after = params.not_before + time::Duration::days(365);
    let key_pair = KeyPair::generate()?;
    let cert = params.signed_by(&key_pair, &ca_cert, &key_pair)?;
    fs::write("certs/localhost.pem", cert.pem())?;
    fs::write("certs/localhost-key.pem", key_pair.serialize_pem())?;
    println!("Generated certs in certs/");
    Ok(())
}
