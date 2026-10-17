use agent_brain::projects::Project;
use agent_brain::routing::{suggest, SuggestRank};

fn make_proj(id: &str, name_vi: &str, aliases: Vec<&str>) -> Project {
    Project {
        id: id.to_string(),
        path: format!("/tmp/{id}"),
        name_vi: name_vi.to_string(),
        aliases: aliases.into_iter().map(|s| s.to_string()).collect(),
        kind: "code".to_string(),
        profile: "work".to_string(),
        repo: None,
        missing: false,
    }
}

#[test]
fn suggest_accent_and_case() {
    let p1 = make_proj("web-ban-hang", "Web bán hàng", vec!["shop"]);
    let p2 = make_proj("app-sale", "App Sale", vec!["đơn hàng"]);
    let projects = vec![p1, p2];

    // "BÁN" matches web-ban-hang (contains on name_vi "Web bán hàng")
    let res = suggest("BÁN", &projects);
    assert_eq!(res.len(), 1);
    assert_eq!(res[0].project.id, "web-ban-hang");
    assert_eq!(res[0].rank, SuggestRank::Contains);

    // "web ban" -> prefix on name_vi
    let res = suggest("web ban", &projects);
    assert_eq!(res.len(), 1);
    assert_eq!(res[0].project.id, "web-ban-hang");
    assert_eq!(res[0].rank, SuggestRank::Prefix);
    assert_eq!(res[0].matched, "Web bán hàng");

    // "shop" -> exact on alias, matched "shop"
    let res = suggest("shop", &projects);
    assert_eq!(res.len(), 1);
    assert_eq!(res[0].project.id, "web-ban-hang");
    assert_eq!(res[0].rank, SuggestRank::Exact);
    assert_eq!(res[0].matched, "shop");

    // "đơn" normalises đ like the CLI -> prefix on alias "đơn hàng"
    let res = suggest("đơn", &projects);
    assert_eq!(res.len(), 1);
    assert_eq!(res[0].project.id, "app-sale");
    assert_eq!(res[0].rank, SuggestRank::Prefix);
    assert_eq!(res[0].matched, "đơn hàng");

    // "don" also normalises to "don" and matches "đơn hàng"
    let res2 = suggest("don", &projects);
    assert_eq!(res2.len(), 1);
    assert_eq!(res2[0].project.id, "app-sale");
}

#[test]
fn suggest_ranking_order() {
    let p1 = make_proj("shop", "Shop", vec![]);
    let p2 = make_proj("shopping", "Shopping", vec![]);
    let p3 = make_proj("my-shop", "My Shop", vec![]);
    let projects = vec![p1, p2, p3];

    let res = suggest("shop", &projects);
    assert_eq!(res.len(), 3);
    assert_eq!(res[0].project.id, "shop");
    assert_eq!(res[0].rank, SuggestRank::Exact);

    assert_eq!(res[1].project.id, "shopping");
    assert_eq!(res[1].rank, SuggestRank::Prefix);

    assert_eq!(res[2].project.id, "my-shop");
    assert_eq!(res[2].rank, SuggestRank::Contains);
}

#[test]
fn suggest_empty_query_lists_all() {
    let p1 = make_proj("proj-b", "Dự án B", vec![]);
    let p2 = make_proj("proj-a", "Dự án A", vec![]);
    let p3 = make_proj("proj-c", "Dự án C", vec![]);
    let projects = vec![p1, p2, p3];

    let res_empty = suggest("", &projects);
    assert_eq!(res_empty.len(), 3);
    // sorted by id: proj-a, proj-b, proj-c
    assert_eq!(res_empty[0].project.id, "proj-a");
    assert_eq!(res_empty[0].rank, SuggestRank::Contains);
    assert_eq!(res_empty[0].matched, "Dự án A");

    assert_eq!(res_empty[1].project.id, "proj-b");
    assert_eq!(res_empty[1].rank, SuggestRank::Contains);
    assert_eq!(res_empty[1].matched, "Dự án B");

    assert_eq!(res_empty[2].project.id, "proj-c");
    assert_eq!(res_empty[2].rank, SuggestRank::Contains);
    assert_eq!(res_empty[2].matched, "Dự án C");

    let res_spaces = suggest("   ", &projects);
    assert_eq!(res_spaces.len(), 3);
    assert_eq!(res_spaces[0].project.id, "proj-a");
    assert_eq!(res_spaces[1].project.id, "proj-b");
    assert_eq!(res_spaces[2].project.id, "proj-c");
}

#[test]
fn suggest_no_match_is_empty() {
    let p1 = make_proj("proj-a", "Dự án A", vec!["alias1"]);
    let projects = vec![p1];

    let res = suggest("xyz", &projects);
    assert!(res.is_empty());
}
