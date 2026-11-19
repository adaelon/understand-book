use super::*;
    #[test]
    fn board_source_window_network_read_keeps_reader_and_revision_until_explicit_open() {
        let f = crate::tests::mu5_tests::Fixture::new();
        let w = f.create(&f.a, &f.x, "source-window");
        let created = f.action(&f.a, &w, "source-window", "boards/create", json!({"title":"原文窗口"}));
        let board = &created["result"];
        let imported = f.action(&f.a, &w, "source-window", "boards/import", json!({"board_id":board["board_id"],"board_revision":board["revision"],
            "source":{"kind":"prebuild","object_id":"speed","object_revision":1,"map_revision":"v1"}}));
        let board = &imported["result"];
        let item = &board["items"][0];
        let mut request = json!({"board_id":board["board_id"],"item_id":item["item_id"],"source_ref_id":item["source_bindings"][0]["source_ref_id"],"anchor_lid":"1.20"});
        let read = f.action(&f.a, &w, "source-window", "boards/source.read", request.clone());
        assert_eq!(read["reader"], w["reader"]);
        assert_eq!(read["revision"], w["revision"]);
        assert_eq!(read["result"]["published_book_ref"], json!(f.x));
        assert_eq!(read["result"]["focus_lid"], "1.20");
        request["lid"] = json!("1.20");
        let opened = f.action(&f.a, &w, "source-window", "boards/source.open", request);
        assert_eq!(opened["reader"]["selection"], "1.20");
        assert!(opened["revision"].as_u64().unwrap() > w["revision"].as_u64().unwrap());
    }
