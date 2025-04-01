use device_id::DeviceIdError;
use rcgen::Certificate;
use rcgen::CertificateParams;
use rcgen::KeyPair;
use sha1::Digest;
use sha1::Sha1;
use std::path::Path;
use std::path::PathBuf;
use time::Duration;
use time::OffsetDateTime;
pub use zeroize::Zeroizing;
#[cfg(feature = "reqwest")]
mod cloud_root_certificate;
#[cfg(feature = "reqwest")]
pub use cloud_root_certificate::*;

pub mod device_id;
pub mod parse_root_certificate;
pub struct PemCertificate {
    pem: x509_parser::pem::Pem,
}

impl PemCertificate {
    pub fn from_pem_file(path: impl AsRef<Path>) -> Result<PemCertificate, CertificateError> {
        let path = path.as_ref();
        let file = std::fs::File::open(path).map_err(|error| CertificateError::IoError {
            error,
            path: path.to_owned(),
        })?;
        let (pem, _) = x509_parser::pem::Pem::read(std::io::BufReader::new(file))?;
        Ok(PemCertificate { pem })
    }

    pub fn from_pem_string(content: &str) -> Result<PemCertificate, CertificateError> {
        let file = std::io::Cursor::new(content.as_bytes());
        let (pem, _) = x509_parser::pem::Pem::read(std::io::BufReader::new(file))?;
        Ok(PemCertificate { pem })
    }

    pub fn subject(&self) -> Result<String, CertificateError> {
        let x509 = PemCertificate::extract_certificate(&self.pem)?;
        Ok(x509.tbs_certificate.subject.to_string())
    }

    pub fn subject_common_name(&self) -> Result<String, CertificateError> {
        let x509 = PemCertificate::extract_certificate(&self.pem)?;
        let subject = x509.tbs_certificate.subject;
        let cn = subject.iter_common_name().next().map(|cn| cn.as_str());

        match cn {
            None => Ok(String::from("")),
            Some(Ok(name)) => Ok(name.to_owned()),
            Some(Err(err)) => Err(PemCertificate::wrap_x509_error(err)),
        }
    }

    pub fn issuer(&self) -> Result<String, CertificateError> {
        let x509 = PemCertificate::extract_certificate(&self.pem)?;
        Ok(x509.tbs_certificate.issuer.to_string())
    }

    pub fn not_before(&self) -> Result<String, CertificateError> {
        let x509 = PemCertificate::extract_certificate(&self.pem)?;
        x509.tbs_certificate
            .validity
            .not_before
            .to_rfc2822()
            .map_err(CertificateError::X509Error)
    }

    pub fn not_after(&self) -> Result<String, CertificateError> {
        let x509 = PemCertificate::extract_certificate(&self.pem)?;
        x509.tbs_certificate
            .validity
            .not_after
            .to_rfc2822()
            .map_err(CertificateError::X509Error)
    }

    pub fn serial(&self) -> Result<String, CertificateError> {
        let x509 = PemCertificate::extract_certificate(&self.pem)?;
        Ok(x509.tbs_certificate.serial.to_string())
    }

    pub fn serial_hex(&self) -> Result<String, CertificateError> {
        let x509 = PemCertificate::extract_certificate(&self.pem)?;
        Ok(format!("{:x}", x509.tbs_certificate.serial))
    }

    pub fn thumbprint(&self) -> Result<String, CertificateError> {
        let bytes = Sha1::digest(&self.pem.contents).as_slice().to_vec();
        let strs: Vec<String> = bytes.iter().map(|b| format!("{:02X}", b)).collect();
        Ok(strs.concat())
    }

    fn extract_certificate(
        pem: &x509_parser::pem::Pem,
    ) -> Result<x509_parser::certificate::X509Certificate, CertificateError> {
        let x509 = pem.parse_x509().map_err(|err| {
            // The x509 error is wrapped into a `nom::Err`
            // and cannot be extracted without pattern matching on that type
            // So one simply extract the error as a string,
            // to avoid a dependency on the `nom` crate.
            let x509_error_string = format!("{}", err);
            CertificateError::X509Error(x509_error_string)
        })?;
        Ok(x509)
    }

    fn wrap_x509_error(err: x509_parser::error::X509Error) -> CertificateError {
        let x509_error_string = format!("{}", err);
        CertificateError::X509Error(x509_error_string)
    }
}

pub enum KeyKind {
    /// Create a new key
    New,
    /// Reuse the existing PEM-encoded key pair
    Reuse { keypair_pem: String },
}

pub struct KeyCertPair {
    certificate: Zeroizing<rcgen::Certificate>,
}

impl KeyCertPair {
    pub fn new_selfsigned_certificate(
        config: &NewCertificateConfig,
        id: &str,
        key_kind: &KeyKind,
    ) -> Result<KeyCertPair, CertificateError> {
        let today = OffsetDateTime::now_utc();
        let not_before = today - Duration::days(1); // Ensure the certificate is valid today
        let params =
            Self::create_selfsigned_certificate_parameters(config, id, key_kind, not_before)?;

        Ok(KeyCertPair {
            certificate: Zeroizing::new(Certificate::from_params(params)?),
        })
    }

    pub fn new_certificate_sign_request(
        config: &NewCertificateConfig,
        id: &str,
        key_kind: &KeyKind,
    ) -> Result<KeyCertPair, CertificateError> {
        // Create Certificate without `not_before` and `not_after` fields
        // as rcgen library will not parse it for certificate signing request
        let params = Self::create_csr_parameters(config, id, key_kind)?;
        Ok(KeyCertPair {
            certificate: Zeroizing::new(Certificate::from_params(params)?),
        })
    }

    fn create_selfsigned_certificate_parameters(
        config: &NewCertificateConfig,
        id: &str,
        key_kind: &KeyKind,
        not_before: OffsetDateTime,
    ) -> Result<CertificateParams, CertificateError> {
        let mut params = Self::create_csr_parameters(config, id, key_kind)?;

        let not_after = not_before + Duration::days(config.validity_period_days.into());
        params.not_before = not_before;
        params.not_after = not_after;

        // IsCa::SelfSignedOnly is rejected by C8Y with "422 Unprocessable Entity"
        params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);

        Ok(params)
    }

    fn create_csr_parameters(
        config: &NewCertificateConfig,
        id: &str,
        key_kind: &KeyKind,
    ) -> Result<CertificateParams, CertificateError> {
        KeyCertPair::check_identifier(id, config.max_cn_size)?;
        let mut distinguished_name = rcgen::DistinguishedName::new();
        distinguished_name.push(rcgen::DnType::CommonName, id);
        distinguished_name.push(rcgen::DnType::OrganizationName, &config.organization_name);
        distinguished_name.push(
            rcgen::DnType::OrganizationalUnitName,
            &config.organizational_unit_name,
        );

        let mut params = CertificateParams::default();
        params.distinguished_name = distinguished_name;

        // ECDSA signing using the P-256 curves and SHA-256 hashing as per RFC 5758
        params.alg = &rcgen::PKCS_ECDSA_P256_SHA256;

        if let KeyKind::Reuse { keypair_pem } = key_kind {
            params.key_pair = Some(KeyPair::from_pem(keypair_pem)?);
        }

        Ok(params)
    }

    pub fn certificate_pem_string(&self) -> Result<String, CertificateError> {
        Ok(self.certificate.serialize_pem()?)
    }

    pub fn private_key_pem_string(&self) -> Result<Zeroizing<String>, CertificateError> {
        Ok(Zeroizing::new(self.certificate.serialize_private_key_pem()))
    }

    pub fn certificate_signing_request_string(&self) -> Result<String, CertificateError> {
        Ok(self.certificate.serialize_request_pem()?)
    }

    fn check_identifier(id: &str, max_cn_size: usize) -> Result<(), CertificateError> {
        Ok(device_id::is_valid_device_id(id, max_cn_size)?)
    }
}

pub fn translate_rustls_error(err: &(dyn std::error::Error + 'static)) -> Option<CertificateError> {
    if let Some(rustls::Error::InvalidCertificate(inner)) = err.downcast_ref::<rustls::Error>() {
        match inner {
            rustls::CertificateError::Expired => Some(CertificateError::CertificateValidationFailure {
                hint: "The server certificate has expired, the time it is being validated for is later than the certificate's `notAfter` time.".into(),
                msg: err.to_string()
            }),

            rustls::CertificateError::NotValidYet => Some(CertificateError::CertificateValidationFailure {
                hint: "The server certificate is not valid yet, the time it is being validated for is earlier than the certificate's `notBefore` time.".into(),
                msg: err.to_string(),
            }),

            _ => Some(CertificateError::CertificateValidationFailure {
                hint: "Server certificate validation error.".into(),
                msg: err.to_string(),
            }),
        }
    } else {
        None
    }
}

#[derive(thiserror::Error, Debug)]
pub enum CertificateError {
    #[error("Could not access {path}: {error}")]
    IoError {
        path: PathBuf,
        error: std::io::Error,
    },

    #[error("Cryptography related error")]
    CryptographyError(#[from] rcgen::Error),

    #[error("PEM file format error")]
    PemError(#[from] x509_parser::error::PEMError),

    #[error("X509 file format error: {0}")]
    X509Error(String), // One cannot use x509_parser::error::X509Error unless one use `nom`.

    #[error("DeviceID Error")]
    InvalidDeviceID(#[from] DeviceIdError),

    #[error("Fail to parse the private key")]
    UnknownPrivateKeyFormat,

    #[error("Could not parse certificate {path}")]
    CertificateParseFailed {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("HTTP Connection Problem: {msg} \nHint: {hint}")]
    CertificateValidationFailure { hint: String, msg: String },

    #[error("Failed to add the certificate to root store")]
    RootStoreAdd,

    #[error(transparent)]
    CertParse(#[from] rustls::Error),

    #[error(transparent)]
    CertParse2(#[from] rustls::pki_types::pem::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub struct NewCertificateConfig {
    pub max_cn_size: usize,
    pub validity_period_days: u32,
    pub organization_name: String,
    pub organizational_unit_name: String,
}

impl Default for NewCertificateConfig {
    fn default() -> Self {
        NewCertificateConfig {
            max_cn_size: 64,
            validity_period_days: 365,
            organization_name: "Thin Edge".into(),
            organizational_unit_name: "Test Device".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::prelude::*;
    use std::error::Error;
    use time::macros::datetime;
    use x509_parser::der_parser::asn1_rs::FromDer;

    impl KeyCertPair {
        fn new_selfsigned_certificate_with_new_key(
            config: &NewCertificateConfig,
            id: &str,
        ) -> Result<KeyCertPair, CertificateError> {
            KeyCertPair::new_selfsigned_certificate(config, id, &KeyKind::New)
        }
    }

    fn pem_of_keypair(keypair: &KeyCertPair) -> PemCertificate {
        let pem_string = keypair
            .certificate_pem_string()
            .expect("Fail to read the certificate PEM");
        PemCertificate::from_pem_string(&pem_string).expect("Fail to decode the certificate PEM")
    }

    fn subject_of_csr(keypair: &KeyCertPair) -> String {
        let csr = keypair
            .certificate_signing_request_string()
            .expect("Failed to read the CSR string");

        let pem = x509_parser::pem::Pem::iter_from_buffer(csr.as_bytes())
            .next()
            .unwrap()
            .expect("Reading PEM block failed");

        x509_parser::certification_request::X509CertificationRequest::from_der(&pem.contents)
            .unwrap()
            .1
            .certification_request_info
            .subject
            .to_string()
    }

    #[test]
    fn self_signed_cert_subject_is_the_device() {
        // Create a certificate with a given subject
        let config = NewCertificateConfig {
            organization_name: "Acme".to_owned(),
            organizational_unit_name: "IoT".to_owned(),
            ..Default::default()
        };
        let id = "device-serial-number";

        let keypair = KeyCertPair::new_selfsigned_certificate_with_new_key(&config, id)
            .expect("Fail to create a certificate");

        // Check the subject
        let pem = pem_of_keypair(&keypair);
        let subject = pem.subject().expect("Fail to extract the subject");
        assert_eq!(subject, "CN=device-serial-number, O=Acme, OU=IoT");
    }

    #[test]
    fn self_signed_cert_common_name_is_the_device_id() {
        // Create a certificate with a given subject
        let config = NewCertificateConfig {
            organization_name: "Acme".to_owned(),
            organizational_unit_name: "IoT".to_owned(),
            ..Default::default()
        };
        let device_id = "device-identifier";

        let keypair = KeyCertPair::new_selfsigned_certificate_with_new_key(&config, device_id)
            .expect("Fail to create a certificate");

        // Check the subject's common_name
        let pem = pem_of_keypair(&keypair);
        let common_name = pem
            .subject_common_name()
            .expect("Fail to extract the common name");
        assert_eq!(common_name, device_id);
    }

    #[test]
    fn self_signed_cert_issuer_is_the_device() {
        // Create a certificate with a given subject
        let config = NewCertificateConfig {
            organization_name: "Acme".to_owned(),
            organizational_unit_name: "IoT".to_owned(),
            ..Default::default()
        };
        let id = "device-serial-number";

        let keypair = KeyCertPair::new_selfsigned_certificate_with_new_key(&config, id)
            .expect("Fail to create a certificate");

        // Check the issuer
        let pem = pem_of_keypair(&keypair);
        let issuer = pem.issuer().expect("Fail to extract the issuer");
        assert_eq!(issuer, "CN=device-serial-number, O=Acme, OU=IoT");
    }

    #[test]
    fn self_signed_cert_no_before_is_birthdate() {
        // Create a certificate with a given birthdate.
        let config = NewCertificateConfig::default();
        let id = "some-id";
        let birthdate = datetime!(2021-03-31 16:39:57 +01:00);

        let params = KeyCertPair::create_selfsigned_certificate_parameters(
            &config,
            id,
            &KeyKind::New,
            birthdate,
        )
        .expect("Fail to get a certificate parameters");

        let keypair = KeyCertPair {
            certificate: Zeroizing::new(
                Certificate::from_params(params).expect("Fail to create a certificate"),
            ),
        };

        // Check the not_before date
        let pem = pem_of_keypair(&keypair);
        let not_before = pem
            .not_before()
            .expect("Fail to extract the not_before date");
        assert_eq!(not_before, "Wed, 31 Mar 2021 15:39:57 +0000");
    }

    #[test]
    fn self_signed_cert_no_after_is_related_to_birthdate() {
        // Create a certificate with a given birthdate.
        let config = NewCertificateConfig {
            validity_period_days: 10,
            ..Default::default()
        };
        let id = "some-id";
        let birthdate = datetime!(2021-03-31 16:39:57 +01:00);

        let params = KeyCertPair::create_selfsigned_certificate_parameters(
            &config,
            id,
            &KeyKind::New,
            birthdate,
        )
        .expect("Fail to get a certificate parameters");

        let keypair = KeyCertPair {
            certificate: Zeroizing::new(
                Certificate::from_params(params).expect("Fail to create a certificate"),
            ),
        };

        // Check the not_after date
        let pem = pem_of_keypair(&keypair);
        let not_after = pem.not_after().expect("Fail to extract the not_after date");
        assert_eq!(not_after, "Sat, 10 Apr 2021 15:39:57 +0000");
    }

    #[test]
    fn create_certificate_sign_request() {
        // Create a certificate with a given birthdate.
        let config = NewCertificateConfig::default();
        let id = "some-id";

        let params = KeyCertPair::create_csr_parameters(&config, id, &KeyKind::New)
            .expect("Fail to get a certificate parameters");

        let keypair = KeyCertPair {
            certificate: Zeroizing::new(
                Certificate::from_params(params).expect("Fail to create a certificate"),
            ),
        };

        // Check the subject
        let subject = subject_of_csr(&keypair);
        assert_eq!(subject, "CN=some-id, O=Thin Edge, OU=Test Device");
    }

    #[test]
    fn check_certificate_thumbprint_b64_decode_sha1() {
        // Create a certificate key pair
        let id = "my-device-id";
        let config = NewCertificateConfig::default();
        let keypair = KeyCertPair::new_selfsigned_certificate_with_new_key(&config, id)
            .expect("Fail to create a certificate");

        // Read the certificate pem
        let pem_string = keypair
            .certificate_pem_string()
            .expect("Fail to read the certificate PEM");
        let pem = PemCertificate::from_pem_string(&pem_string)
            .expect("Fail to decode the certificate PEM");

        // Compute the thumbprint of the certificate using this crate
        let thumbprint = pem.thumbprint().expect("Fail to compute the thumbprint");

        // Compute the expected thumbprint of the certificate using base64 and sha1
        // Remove new line and carriage return characters
        let cert_cont = pem_string.replace(&['\r', '\n'][..], "");

        // Read the certificate contents, except the header and footer
        let header_len = "-----BEGIN CERTIFICATE-----".len();
        let footer_len = "-----END CERTIFICATE-----".len();

        // just decode the key contents
        let b64_bytes = BASE64_STANDARD
            .decode(&cert_cont[header_len..cert_cont.len() - footer_len])
            .unwrap();
        let expected_thumbprint = format!("{:x}", sha1::Sha1::digest(b64_bytes));

        // compare the two thumbprints
        assert_eq!(thumbprint, expected_thumbprint.to_uppercase());
    }

    #[test]
    fn check_thumbprint_static_certificate() {
        let cert_content = include_str!("./test_certificate.txt");
        let expected_thumbprint = "860218AD0A996004449521E2713C28F67B5EA580";

        let pem = PemCertificate::from_pem_string(cert_content).expect("Reading PEM failed");
        let thumbprint = pem.thumbprint().expect("Extracting thumbprint failed");
        assert_eq!(thumbprint, expected_thumbprint);
    }

    #[test]
    fn check_translate_rustls_error() -> Result<(), anyhow::Error> {
        let expired_error = rustls::Error::InvalidCertificate(rustls::CertificateError::Expired);
        let expired_error2: Box<dyn Error> = expired_error.clone().into();
        let translated_error = translate_rustls_error(expired_error2.as_ref());

        println!("plaintext error: {expired_error}");

        println!(
            "anyhow-formatted error: {:?}",
            anyhow::Error::from(expired_error)
        );

        if let Some(inner) = translated_error {
            println!("translated error: {inner}");
            println!("anyhow-formatted error: {:?}", anyhow::Error::from(inner));
        }

        Ok(())
    }
}
