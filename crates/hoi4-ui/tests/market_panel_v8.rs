use hoi4_ui::market_panel::{
    GoodCategory, GoodEntry, GoodFlowSource, GoodSupplySourceEntry, GoodSupplySourceKind,
    MarketActionEntry, MarketBlocMemberEntry, MarketBlocPanelData, MarketPanelData,
    MarketSubjectEntry,
};

fn rubber_entry() -> GoodEntry {
    GoodEntry {
        id: "rubber".into(),
        name: "橡胶".into(),
        category: GoodCategory::RawMaterial,
        price: 20.0,
        base_price: 10.0,
        supply: 18.0,
        demand: 30.0,
        traded: 18.0,
        stockpile: 0.0,
        stockpile_coverage_days: 0.0,
        unmet_demand: 12.0,
        domestic_production: 0.0,
        stockpile_draw: 0.0,
        building_input_demand: 30.0,
        pop_consumption_demand: 0.0,
        military_order_demand: 0.0,
        supply_sources: vec![
            GoodSupplySourceEntry {
                kind: GoodSupplySourceKind::Subject,
                label: "MAL 殖民/傀儡贡献".into(),
                amount: 18.0,
            },
            GoodSupplySourceEntry {
                kind: GoodSupplySourceKind::WorldSpot,
                label: "世界现货".into(),
                amount: 2.0,
            },
        ],
        producers: vec![],
        consumers: vec![GoodFlowSource {
            name: "飞机工厂".into(),
            amount: 30.0,
        }],
        government_orders: vec![],
        construction_demand: 0.0,
        imports: 18.0,
        exports: 0.0,
        is_blockaded: false,
        affected_buildings: vec![],
        affected_pop_classes: vec![],
        upstream_goods: vec![],
        downstream_goods: vec!["rubber_parts".into()],
        paid_rm: 0.0,
        clearing_fulfilled: 18.0,
        clearing_unmet: 12.0,
        clearing_shortage_ratio: 0.4,
    }
}

fn panel_data() -> MarketPanelData {
    MarketPanelData {
        goods: vec![rubber_entry()],
        bloc: Some(MarketBlocPanelData {
            name: "英帝国优惠体系".into(),
            kind: "帝国优惠".into(),
            leader_tag: "ENG".into(),
            members: vec![
                MarketBlocMemberEntry {
                    tag: "ENG".into(),
                    relation: "领导国".into(),
                    market_access: 1.0,
                    contribution_supply_value_rm: 100.0,
                    contribution_demand_value_rm: 120.0,
                    strategic_goods: vec![],
                },
                MarketBlocMemberEntry {
                    tag: "MAL".into(),
                    relation: "傀儡".into(),
                    market_access: 1.0,
                    contribution_supply_value_rm: 180.0,
                    contribution_demand_value_rm: 10.0,
                    strategic_goods: vec![GoodFlowSource {
                        name: "橡胶".into(),
                        amount: 18.0,
                    }],
                },
            ],
            internal_trade_value_gbp: 12.0,
            external_trade_value_gbp: 2.0,
        }),
        exchange_rate: 12.5,
        cash_rm: 1_000_000.0,
        total_shortage_value_rm: 240.0,
        total_import_value_gbp: 28.8,
        total_export_value_gbp: 0.0,
        pop_needs_fulfillment: 0.95,
        military_supply_pressure: 0.2,
        subjects: vec![MarketSubjectEntry {
            tag: "MAL".into(),
            autonomy_level: "傀儡".into(),
            master_resource_share: 0.5,
            resource_contribution: vec![GoodFlowSource {
                name: "橡胶".into(),
                amount: 18.0,
            }],
            fiscal_contribution_gbp: 0.9,
            risk: "高抽取会压低自治度进展并提高殖民风险".into(),
        }],
        actions: vec![MarketActionEntry {
            title: "扩张 橡胶 上游链".into(),
            description: "检查 MAL 橡胶、世界现货和飞机工厂输入。".into(),
            related_good_id: Some("rubber".into()),
            priority: 0,
        }],
        alerts: vec![],
    }
}

#[test]
fn market_panel_shows_bloc_members() {
    let data = panel_data();
    let bloc = data.bloc.expect("bloc data should exist");
    assert_eq!(bloc.name, "英帝国优惠体系");
    assert!(bloc.members.iter().any(|member| member.tag == "MAL"));
}

#[test]
fn market_panel_splits_domestic_bloc_subject_world_supply() {
    let data = panel_data();
    let rubber = &data.goods[0];
    assert!(
        rubber
            .supply_sources
            .iter()
            .any(|source| source.kind == GoodSupplySourceKind::Subject
                && source.label.contains("MAL"))
    );
    assert!(rubber
        .supply_sources
        .iter()
        .any(|source| source.kind == GoodSupplySourceKind::WorldSpot));
}

#[test]
fn market_panel_explains_colonial_dependency() {
    let data = panel_data();
    let subject = data
        .subjects
        .iter()
        .find(|subject| subject.tag == "MAL")
        .expect("MAL subject should be visible");
    assert_eq!(subject.autonomy_level, "傀儡");
    assert!(subject
        .resource_contribution
        .iter()
        .any(|row| row.name == "橡胶" && row.amount > 0.0));
}

#[test]
fn market_panel_actions_reference_specific_goods() {
    let data = panel_data();
    let action = data.actions.first().expect("action should exist");
    assert_eq!(action.related_good_id.as_deref(), Some("rubber"));
    assert!(action.description.contains("MAL") || action.description.contains("橡胶"));
}
