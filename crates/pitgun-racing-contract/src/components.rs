//! Versioned Racing component-capability contracts.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ComponentCapabilityProfileVersion {
    #[serde(rename = "pitgun.racing-component-capabilities/v1")]
    V1,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ResolvedVehicleCapabilitiesVersion {
    #[serde(rename = "pitgun.racing-resolved-vehicle-capabilities/v1")]
    V1,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VehicleComponentKind {
    AerodynamicPackage,
    Chassis,
    PowerUnit,
    TireSpecification,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VehicleCapability {
    AdjustableDownforce,
    AdjustableGearRatio,
    TurboConfiguration,
    EnergyDeployment,
    EnergyRecovery,
}

impl VehicleCapability {
    #[must_use]
    pub const fn required_component_kind(self) -> VehicleComponentKind {
        match self {
            Self::AdjustableDownforce => VehicleComponentKind::AerodynamicPackage,
            Self::AdjustableGearRatio
            | Self::TurboConfiguration
            | Self::EnergyDeployment
            | Self::EnergyRecovery => VehicleComponentKind::PowerUnit,
        }
    }

    pub const ALL: [Self; 5] = [
        Self::AdjustableDownforce,
        Self::AdjustableGearRatio,
        Self::TurboConfiguration,
        Self::EnergyDeployment,
        Self::EnergyRecovery,
    ];
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentCapabilityDefinitionV1 {
    pub kind: VehicleComponentKind,
    pub component_id: String,
    pub capabilities: Vec<VehicleCapability>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentCapabilityProfileV1 {
    pub schema_version: ComponentCapabilityProfileVersion,
    pub components: Vec<ComponentCapabilityDefinitionV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstalledVehicleComponentV1 {
    pub kind: VehicleComponentKind,
    pub component_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnavailableVehicleCapabilityV1 {
    pub capability: VehicleCapability,
    pub required_component_kind: VehicleComponentKind,
    pub installed_component_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedVehicleCapabilitiesV1 {
    pub schema_version: ResolvedVehicleCapabilitiesVersion,
    pub baseline_vehicle_id: String,
    pub components: Vec<InstalledVehicleComponentV1>,
    pub supported_capabilities: Vec<VehicleCapability>,
    pub unavailable_capabilities: Vec<UnavailableVehicleCapabilityV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ComponentCapabilityError {
    EmptyProfile,
    NonCanonicalComponents,
    NonCanonicalCapabilities(String),
    ComponentCoverageMismatch,
    MissingInstalledComponent(VehicleComponentKind),
    UnknownInstalledComponent {
        kind: VehicleComponentKind,
        component_id: String,
    },
}

impl fmt::Display for ComponentCapabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyProfile => formatter.write_str("component capability profile is empty"),
            Self::NonCanonicalComponents => formatter.write_str(
                "component capability definitions must be unique and canonically ordered",
            ),
            Self::NonCanonicalCapabilities(component_id) => write!(
                formatter,
                "component {component_id:?} capabilities must be unique and canonically ordered"
            ),
            Self::ComponentCoverageMismatch => formatter.write_str(
                "component capability profile does not exactly cover catalog components",
            ),
            Self::MissingInstalledComponent(kind) => {
                write!(formatter, "vehicle composition is missing {kind:?}")
            }
            Self::UnknownInstalledComponent { kind, component_id } => write!(
                formatter,
                "unknown installed {kind:?} component {component_id:?}"
            ),
        }
    }
}

impl std::error::Error for ComponentCapabilityError {}

impl ComponentCapabilityProfileV1 {
    pub fn validate_against(
        &self,
        expected: &BTreeMap<VehicleComponentKind, BTreeSet<String>>,
    ) -> Result<(), ComponentCapabilityError> {
        if self.components.is_empty() {
            return Err(ComponentCapabilityError::EmptyProfile);
        }
        if self.components.windows(2).any(|entries| {
            (entries[0].kind, entries[0].component_id.as_str())
                >= (entries[1].kind, entries[1].component_id.as_str())
        }) {
            return Err(ComponentCapabilityError::NonCanonicalComponents);
        }
        for component in &self.components {
            if component.component_id.trim().is_empty()
                || component
                    .capabilities
                    .windows(2)
                    .any(|values| values[0] >= values[1])
            {
                return Err(ComponentCapabilityError::NonCanonicalCapabilities(
                    component.component_id.clone(),
                ));
            }
        }
        let actual = self.components.iter().fold(
            BTreeMap::<VehicleComponentKind, BTreeSet<String>>::new(),
            |mut entries, component| {
                entries
                    .entry(component.kind)
                    .or_default()
                    .insert(component.component_id.clone());
                entries
            },
        );
        if &actual != expected {
            return Err(ComponentCapabilityError::ComponentCoverageMismatch);
        }
        Ok(())
    }

    pub fn resolve(
        &self,
        baseline_vehicle_id: &str,
        installed: BTreeMap<VehicleComponentKind, String>,
    ) -> Result<ResolvedVehicleCapabilitiesV1, ComponentCapabilityError> {
        let definitions = self
            .components
            .iter()
            .map(|definition| {
                (
                    (definition.kind, definition.component_id.as_str()),
                    definition,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut components = Vec::new();
        let mut supported = BTreeSet::new();
        for kind in [
            VehicleComponentKind::AerodynamicPackage,
            VehicleComponentKind::Chassis,
            VehicleComponentKind::PowerUnit,
            VehicleComponentKind::TireSpecification,
        ] {
            let component_id = installed
                .get(&kind)
                .ok_or(ComponentCapabilityError::MissingInstalledComponent(kind))?;
            let definition = definitions
                .get(&(kind, component_id.as_str()))
                .ok_or_else(|| ComponentCapabilityError::UnknownInstalledComponent {
                    kind,
                    component_id: component_id.clone(),
                })?;
            supported.extend(definition.capabilities.iter().copied());
            components.push(InstalledVehicleComponentV1 {
                kind,
                component_id: component_id.clone(),
            });
        }
        let supported_capabilities = supported.iter().copied().collect::<Vec<_>>();
        let unavailable_capabilities = VehicleCapability::ALL
            .into_iter()
            .filter(|capability| !supported.contains(capability))
            .map(|capability| {
                let required_component_kind = capability.required_component_kind();
                UnavailableVehicleCapabilityV1 {
                    capability,
                    required_component_kind,
                    installed_component_id: installed
                        .get(&required_component_kind)
                        .expect("all component kinds were resolved above")
                        .clone(),
                }
            })
            .collect();
        Ok(ResolvedVehicleCapabilitiesV1 {
            schema_version: ResolvedVehicleCapabilitiesVersion::V1,
            baseline_vehicle_id: baseline_vehicle_id.to_string(),
            components,
            supported_capabilities,
            unavailable_capabilities,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> ComponentCapabilityProfileV1 {
        ComponentCapabilityProfileV1 {
            schema_version: ComponentCapabilityProfileVersion::V1,
            components: vec![
                ComponentCapabilityDefinitionV1 {
                    kind: VehicleComponentKind::AerodynamicPackage,
                    component_id: "basic".to_string(),
                    capabilities: vec![VehicleCapability::AdjustableDownforce],
                },
                ComponentCapabilityDefinitionV1 {
                    kind: VehicleComponentKind::Chassis,
                    component_id: "default".to_string(),
                    capabilities: vec![],
                },
                ComponentCapabilityDefinitionV1 {
                    kind: VehicleComponentKind::PowerUnit,
                    component_id: "v8".to_string(),
                    capabilities: vec![VehicleCapability::AdjustableGearRatio],
                },
                ComponentCapabilityDefinitionV1 {
                    kind: VehicleComponentKind::TireSpecification,
                    component_id: "medium".to_string(),
                    capabilities: vec![],
                },
            ],
        }
    }

    fn installed() -> BTreeMap<VehicleComponentKind, String> {
        BTreeMap::from([
            (
                VehicleComponentKind::AerodynamicPackage,
                "basic".to_string(),
            ),
            (VehicleComponentKind::Chassis, "default".to_string()),
            (VehicleComponentKind::PowerUnit, "v8".to_string()),
            (
                VehicleComponentKind::TireSpecification,
                "medium".to_string(),
            ),
        ])
    }

    #[test]
    fn resolution_exposes_supported_and_explains_unavailable_controls() {
        let resolved = profile()
            .resolve("classic", installed())
            .expect("resolution");
        assert_eq!(
            resolved.supported_capabilities,
            vec![
                VehicleCapability::AdjustableDownforce,
                VehicleCapability::AdjustableGearRatio,
            ]
        );
        let energy = resolved
            .unavailable_capabilities
            .iter()
            .find(|item| item.capability == VehicleCapability::EnergyDeployment)
            .expect("energy deployment explanation");
        assert_eq!(
            energy.required_component_kind,
            VehicleComponentKind::PowerUnit
        );
        assert_eq!(energy.installed_component_id, "v8");
    }

    #[test]
    fn authored_profiles_fail_closed_on_noncanonical_or_incomplete_coverage() {
        let expected = installed()
            .into_iter()
            .map(|(kind, id)| (kind, BTreeSet::from([id])))
            .collect::<BTreeMap<_, _>>();
        profile()
            .validate_against(&expected)
            .expect("valid profile");

        let mut duplicate = profile();
        duplicate
            .components
            .insert(1, duplicate.components[0].clone());
        assert_eq!(
            duplicate.validate_against(&expected),
            Err(ComponentCapabilityError::NonCanonicalComponents)
        );

        let mut incomplete = expected;
        incomplete
            .get_mut(&VehicleComponentKind::PowerUnit)
            .expect("power-unit set")
            .insert("hybrid".to_string());
        assert_eq!(
            profile().validate_against(&incomplete),
            Err(ComponentCapabilityError::ComponentCoverageMismatch)
        );
    }
}
