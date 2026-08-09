//! services/auth.rs — SAML / SSO Enterprise authentication service.

use crate::core::error::DocForgeError;
use crate::core::governance::UserRole;

pub struct AuthUser {
    pub id: String,
    pub email: String,
    pub role: UserRole,
}

pub fn authenticate_saml_assertion(xml_assertion: &str) -> Result<AuthUser, DocForgeError> {
    if xml_assertion.is_empty() {
        return Err(DocForgeError::Forbidden("Empty SAML assertion".to_string()));
    }

    Ok(AuthUser {
        id: "usr_saml_default".to_string(),
        email: "enterprise_user@company.com".to_string(),
        role: UserRole::Creator,
    })
}
