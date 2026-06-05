use hoi4_ui::{
    construction_v6_panel::{
        construction_v9_secondary_tabs, ConstructionQueueV6Entry, ConstructionV6Command,
        ConstructionV6PanelData, InvestmentPoolV6Data,
    },
    finance_panel::economy_v9_secondary_tabs,
    market_panel::{
        market_v9_secondary_tabs, GoodCategory, GoodEntry, MarketActionEntry, MarketPanelData,
    },
    pop_panel::pop_v9_secondary_tabs,
};

#[test]
fn g7_economy_finance_construction_market_and_pop_tabs_are_complete() {
    assert_eq!(
        economy_v9_secondary_tabs(),
        &[
            ("overview", "总览"),
            ("gdp", "GDP"),
            ("primary", "一产"),
            ("secondary", "二产"),
            ("tertiary", "三产"),
            ("employment", "就业"),
            ("investment", "投资"),
            ("trade", "贸易影响"),
            ("diagnostics", "诊断"),
        ]
    );
    assert_eq!(
        construction_v9_secondary_tabs(),
        &[
            ("overview", "总览"),
            ("queue", "队列"),
            ("catalog", "建筑目录"),
            ("primary", "一产建筑"),
            ("secondary", "二产建筑"),
            ("tertiary", "三产建筑"),
            ("infrastructure", "基础设施"),
            ("military", "军事设施"),
            ("bottlenecks", "瓶颈"),
            ("auto_build", "自动建设"),
        ]
    );
    assert_eq!(
        market_v9_secondary_tabs(),
        &[
            ("overview", "总览"),
            ("shortages", "短缺"),
            ("goods", "商品"),
            ("industry_chain", "产业链"),
            ("trade", "进口出口"),
            ("market_bloc", "市场圈"),
            ("subjects", "殖民/傀儡供给"),
            ("actions", "行动建议"),
        ]
    );
    assert_eq!(
        pop_v9_secondary_tabs(),
        &[
            ("national", "全国"),
            ("states", "州人口"),
            ("classes", "阶层"),
            ("employment", "就业"),
            ("income", "收入"),
            ("needs", "消费需求"),
            ("education", "教育技能"),
            ("satisfaction", "满意度"),
        ]
    );
}

#[test]
fn g4_construction_queue_dto_exposes_capacity_bottlenecks_and_commands() {
    let data = ConstructionV6PanelData {
        entries: vec![],
        queue: vec![ConstructionQueueV6Entry {
            building_key: "steel_mill".into(),
            building_name: "钢铁厂".into(),
            state_id: 51,
            state_name: "莱茵兰".into(),
            current_level: 1,
            target_level: 2,
            recipe_cp_cost: 9_800.0,
            recipe_funds_rm: 520_000_000.0,
            recipe_materials_summary: "钢材 160，机械 65".into(),
            recipe_labor: 1_180,
            recipe_engineering: 90,
            recipe_region_summary: "城市州".into(),
            progress: 0.35,
            funding_source_label: "政府预算".into(),
            owner_on_completion_label: "国有".into(),
            paid_funds_rm: 50_000_000.0,
            budget_needed_rm: 360_000_000.0,
            material_fulfillment: 0.85,
            fund_ratio: 1.0,
            priority: 1,
            weight: 1.5,
            paused: false,
            allocated_cp: 40.0,
            effective_cp: 34.0,
            blocked_cp: 6.0,
            bottleneck_label: "materials".into(),
            estimated_days: Some(18),
        }],
        buildable_catalog: vec![],
        active_construction_key: Some("steel_mill".into()),
        available_cp: 50.0,
        total_cp: 100.0,
        national_admin_cp: 20.0,
        construction_sector_cp: 35.0,
        regional_labor_cp: 25.0,
        engineering_equipment_cp: 20.0,
        finance_cp: 90.0,
        material_cp: 80.0,
        allocated_cp: 50.0,
        idle_cp: 25.0,
        blocked_cp: 25.0,
        gdp_gbp: 1_000_000_000.0,
        gdp_growth_yoy: 2.5,
        construction_spend_rm: 500_000.0,
        unemployment_rate: 0.04,
        military_orders_rm: 750_000.0,
        mefo_risk: 0.12,
        auto_build_enabled: true,
        auto_build_explanations: vec![],
        investment_pool: InvestmentPoolV6Data::default(),
    };

    let item = &data.queue[0];
    assert!(data.national_admin_cp > 0.0);
    assert!(data.construction_sector_cp > 0.0);
    assert!(data.regional_labor_cp > 0.0);
    assert!(data.engineering_equipment_cp > 0.0);
    assert!(data.finance_cp > 0.0);
    assert!(data.material_cp > 0.0);
    assert!(item.allocated_cp > item.effective_cp);
    assert!(item.blocked_cp > 0.0);
    assert_eq!(item.estimated_days, Some(18));

    let commands = [
        ConstructionV6Command::ToggleAutoBuild(true),
        ConstructionV6Command::MoveUp(0),
        ConstructionV6Command::MoveDown(0),
        ConstructionV6Command::Remove(0),
        ConstructionV6Command::ToggleProjectPaused(0, true),
        ConstructionV6Command::SetProjectPriority {
            idx: 0,
            priority: 2,
        },
        ConstructionV6Command::SetProjectWeight {
            idx: 0,
            weight: 1.25,
        },
        ConstructionV6Command::StartConstructionMode {
            building_key: "steel_mill".into(),
        },
    ];
    assert_eq!(commands.len(), 8);
}

#[test]
fn g5_shortage_actions_carry_drill_down_targets() {
    let data = MarketPanelData {
        goods: vec![GoodEntry {
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
            supply_sources: vec![],
            producers: vec![],
            consumers: vec![],
            government_orders: vec![],
            construction_demand: 0.0,
            imports: 18.0,
            exports: 0.0,
            is_blockaded: true,
            affected_buildings: vec![],
            affected_pop_classes: vec![],
            affected_demand_buckets: vec![],
            upstream_goods: vec!["天然橡胶".into()],
            downstream_goods: vec!["飞机工厂".into()],
            shortage_reason: "橡胶进口受阻，飞机工厂投入不足".into(),
            actionable_fixes: vec![MarketActionEntry {
                title: "恢复橡胶进口通道".into(),
                description: "打开进口出口或市场圈面板处理封锁线路。".into(),
                related_good_id: Some("rubber".into()),
                priority: 0,
            }],
            paid_rm: 0.0,
            clearing_fulfilled: 18.0,
            clearing_unmet: 12.0,
            clearing_shortage_ratio: 0.4,
        }],
        bloc: None,
        exchange_rate: 12.5,
        cash_rm: 1_000_000.0,
        total_shortage_value_rm: 240.0,
        total_import_value_gbp: 28.8,
        total_export_value_gbp: 0.0,
        pop_needs_fulfillment: 0.95,
        military_supply_pressure: 0.2,
        subjects: vec![],
        actions: vec![MarketActionEntry {
            title: "恢复橡胶进口通道".into(),
            description: "进入进口出口面板并查看橡胶详情。".into(),
            related_good_id: Some("rubber".into()),
            priority: 0,
        }],
        alerts: vec![],
    };

    let action = data.actions.first().expect("shortage action exists");
    assert_eq!(action.related_good_id.as_deref(), Some("rubber"));
    assert!(data
        .goods
        .iter()
        .any(|good| good.id == "rubber" && good.name == "橡胶"));
    assert!(data.goods[0].is_blockaded);
    assert!(data.goods[0].imports > 0.0);
    assert!(data.goods[0]
        .downstream_goods
        .iter()
        .any(|name| name == "飞机工厂"));
}

#[test]
fn g6_public_gate_labels_are_player_visible_chinese_without_internal_ids() {
    let labels = economy_v9_secondary_tabs()
        .iter()
        .chain(construction_v9_secondary_tabs().iter())
        .chain(market_v9_secondary_tabs().iter())
        .chain(pop_v9_secondary_tabs().iter())
        .map(|(_, label)| *label);

    for label in labels {
        assert!(!label.contains("STATE_"));
        assert!(!label.contains("State "));
        assert!(!label.contains("???"));
        assert!(!label.contains("overview"));
        assert!(!label.contains("actions"));
    }
}
