/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs LLC <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only
 *
 */

pub mod logo;
use crate::{LogoCache, Server, manager::application::Resource};
use bytes::Bytes;
use logo::{LogoLoadError, LogoSource};
use registry::schema::structs::{Domain, Tenant};
use utils::outbound::OutboundPolicy;

impl Server {
    pub async fn logo_resource(&self, domain: &str) -> trc::Result<Option<Resource<Bytes>>> {
        let mut domain = psl::domain_str(domain).unwrap_or(domain);
        let logo_cache = { self.inner.data.logos.lock().get(domain).cloned() };
        if let Some(logo) = logo_cache {
            return Ok(logo.data);
        }

        let mut logo_source = None;
        let mut logo_policy = OutboundPolicy::Trusted;
        let mut domain_id = u32::MAX;
        let mut tenant_id = None;
        if let Some((d_id, t_id)) = self.domain(domain).await?.map(|d| (d.id, d.id_tenant))
            && let Some(domain_record) = self.registry().object::<Domain>(d_id.into()).await?
        {
            if domain_record.logo.is_some() {
                logo_policy = OutboundPolicy::for_tenant(domain_record.member_tenant_id);
            }
            logo_source = domain_record.logo;
            domain_id = d_id;
            tenant_id = t_id;

            if logo_source.is_none()
                && let Some(tenant_id) = tenant_id
                && let Some(tenant_logo) = self
                    .registry()
                    .object::<Tenant>(tenant_id.into())
                    .await?
                    .and_then(|t| t.logo)
            {
                logo_policy = OutboundPolicy::PublicOnly;
                logo_source = Some(tenant_logo);
            }
        } else {
            domain = "*";
        }

        if logo_source.is_none()
            && let Some(default_logo) = self.default_logo()
        {
            let logo = {
                self.inner
                    .data
                    .logos
                    .lock()
                    .get("*")
                    .map(|logo| logo.data.clone())
            };
            if let Some(logo) = logo {
                if domain != "*" {
                    self.inner.data.logos.lock().insert(
                        domain.into(),
                        LogoCache {
                            domain_id,
                            tenant_id,
                            data: logo.clone(),
                        },
                    );
                }
                return Ok(logo);
            }
            logo_source = Some(default_logo);
        }

        let logo = match logo_source {
            Some(logo_source) => match LogoSource::parse(&logo_source, logo_policy) {
                Ok(source) => source.load(logo_policy).await.map(Some),
                Err(err) => Err(LogoLoadError::Rejected(err)),
            },
            None => Ok(None),
        };
        let logo = match logo {
            Ok(logo) => logo,
            Err(err) => {
                let is_transient = err.is_transient();
                trc::error!(trc::Error::from(err).ctx(trc::Key::Domain, domain.to_string()));
                if is_transient {
                    return Ok(None);
                }
                None
            }
        };

        self.inner.data.logos.lock().insert(
            domain.into(),
            LogoCache {
                domain_id,
                tenant_id,
                data: logo.clone(),
            },
        );

        Ok(logo)
    }

    fn default_logo(&self) -> Option<String> {
        self.core.logo.clone()
    }
}
