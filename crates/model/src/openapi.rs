//! OpenAPI schemas for the types whose JSON is written by hand.
//!
//! [`Length`], [`Area`] and [`Stages`] have their own serde code, so their
//! schemas are written here to match it; [`NodeKind`]'s is written here
//! because a derived one leaves its `kind` field undescribed. Only built with the `openapi`
//! feature, for the API's `openapi.json`.

use utoipa::openapi::RefOr;
use utoipa::openapi::schema::{Object, ObjectBuilder, OneOfBuilder, Schema, SchemaType, Type};
use utoipa::{PartialSchema, ToSchema};

use crate::{Area, Length, NodeKind, Stages};

impl PartialSchema for Length {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .schema_type(SchemaType::Type(Type::Number))
            .multiple_of(Some(0.1))
            .description(Some(
                "A length in millimetres, with at most one decimal (720, 3.2). \
                 More decimals are rejected, never rounded.",
            ))
            .examples([serde_json::json!(720), serde_json::json!(3.2)])
            .into()
    }
}

impl ToSchema for Length {}

impl PartialSchema for Area {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .schema_type(SchemaType::Type(Type::Number))
            .description(Some("An area in square millimetres."))
            .into()
    }
}

impl ToSchema for Area {}

impl PartialSchema for Stages {
    fn schema() -> RefOr<Schema> {
        OneOfBuilder::new()
            .item(
                ObjectBuilder::new()
                    .schema_type(SchemaType::Type(Type::Integer))
                    .enum_values(Some([2, 3]))
                    .description(Some("At most this many stages.")),
            )
            .item(
                ObjectBuilder::new()
                    .schema_type(SchemaType::Type(Type::String))
                    .enum_values(Some(["unlimited"]))
                    .description(Some("No limit on the number of stages.")),
            )
            .description(Some(
                "The maximum number of cutting stages: 2, 3 or \"unlimited\".",
            ))
            .into()
    }
}

impl ToSchema for Stages {}

impl PartialSchema for NodeKind {
    fn schema() -> RefOr<Schema> {
        // The variants' doc comments, as `#[derive(ToSchema)]` would use them.
        let variant = |kind: &str, description: &str| {
            ObjectBuilder::new()
                .description(Some(description))
                .property(
                    "kind",
                    ObjectBuilder::new()
                        .schema_type(SchemaType::Type(Type::String))
                        .enum_values(Some([kind]))
                        .description(Some(
                            "What this rectangle is: `piece` (the panel, or a piece cut \
                             further), `part`, `offcut` (waste), `leftover` (the reusable \
                             remainder of the last panel) or `trim` (a strip cut off an edge \
                             of the panel first).",
                        )),
                )
                .required("kind")
        };
        let part: Object = variant("part", "A finished part.")
            .property(
                "part_id",
                ObjectBuilder::new()
                    .schema_type(SchemaType::Type(Type::String))
                    .description(Some("The name of the part.")),
            )
            .required("part_id")
            .property(
                "rotated",
                ObjectBuilder::new()
                    .schema_type(SchemaType::Type(Type::Boolean))
                    .description(Some(
                        "Whether the part is turned 90°: its length lies along the panel's width.",
                    )),
            )
            .required("rotated")
            .build();
        OneOfBuilder::new()
            .item(variant(
                "piece",
                "The panel or an intermediate piece, cut further into its children.",
            ))
            .item(part)
            .item(variant("offcut", "Waste."))
            .item(variant(
                "leftover",
                "The reusable remainder of the last panel, beyond its last first-stage cut.",
            ))
            .item(variant(
                "trim",
                "A strip cut off an edge of the panel before any other cut (a trim *Cut off \
                 first*). Trims are children of the panel, at stage 0, before the trimmed \
                 panel, which is a piece at stage 0 too.",
            ))
            .description(Some("The kind of a rectangle in the cutting tree."))
            .into()
    }
}

impl ToSchema for NodeKind {}
