//! Static atlas lookup tables (material-id → atlas, name-based overrides).

pub(super) fn runtime_atlas_for(name: &str, material_id: &str) -> Option<&'static str> {
    match material_id {
        "04f5524815177fe408f9529a451cd50b"
        | "4f58ebaf253ff4341a0acfb2cdf671e6"
        | "84749b95c69414bd28c174439810c2b0"
        | "9148d367fdd7f4e5382e2b9cdf74b461"
        | "a286b652d38de4df384036482abc0571"
        | "d96ee5bd8db944803a0071fc972963a1" => Some("Ingame_Characters_Sheet_01.png"),
        "0bc3a371695e64987907110f53db83ec"
        | "1f3758e86c0414579989dc55480b23bc"
        | "a96fc7a314a89c041bb1a95fd4c281bf"
        | "e0b533defb69748799a56d4ee3b4260b" => Some("IngameAtlas2.png"),
        "38fb36d9f174d40fe859d55deb429e95"
        | "72a903c5f189843248f4878232222af4"
        | "c2c38ca20a8d040139cb7369bf7be51e"
        | "c6cc840a754074fe88e9517644258dc2"
        | "e02abde8d05ec499e9a2ba7f4850c971"
        | "eba8c92c7583e4309be6ad3f5e17e27e"
        | "fa87a551483ef4ba690410a25612e993" => Some("IngameAtlas3.png"),
        "4f53843aa7627f441a5d9e797e1745d9"
        | "51f3931e706115d468eca7f64035a4df"
        | "7844d45e898ea1441a473a80684bf4c4"
        | "89bbb403e054f204395edd3b8fea8241"
        | "bb7f816eb0cbb9f4584de6251fbc6eec"
        | "f6f09bead2bfb8c47957022817a31c85"
        | "f8cf0aa9b5c55c3469100b3a7044d86e" => Some("IngameAtlas.png"),
        "bfa953ce5fc274b6faa59deca6579361" | "f300c561f75e74380a11f80d4d2647f3" => {
            Some("Ingame_Sheet_04.png")
        }
        "1dc9819db44b840edb8cdec9ef7b80c2" => {
            if name == "LevelRowUnlockPanel" {
                Some("Ingame_Sheet_04.png")
            } else {
                Some("Ingame_Characters_Sheet_01.png")
            }
        }
        "20936c462fac24dbb967c450f9cb0cb4" => {
            if name == "GridCell" {
                Some("IngameAtlas2.png")
            } else {
                Some("Ingame_Characters_Sheet_01.png")
            }
        }
        "32e759dd981a043fa8fbcfd4997143ea" => match name {
            "DailyChallengeDialog" | "LeaderboardDialog" | "SeasonEndDialog" | "SnoutCoinShop" => {
                Some("Ingame_Sheet_04.png")
            }
            _ => Some("Ingame_Characters_Sheet_01.png"),
        },
        _ => None,
    }
}

pub(super) fn preferred_runtime_sprite_id(name: &str) -> Option<&'static str> {
    match name {
        "AskAboutNotifications" => Some("ab0c6536-dfc1-46a1-8276-59280b355188"),
        "CakeRaceReplayEntry" => Some("a6cac51f-48ca-46da-b2e0-35cb3eacc819"),
        "CoinSalePopup" | "CrateCrazePopup" => Some("d37f6015-afdb-484e-b57f-451218f82ac2"),
        "ConfirmationErrorDialog"
        | "NoFreeSlotsPopup"
        | "RewardPopup"
        | "SandboxUnlock"
        | "VideoNotFoundDialog" => Some("690f29d0-ee21-4724-b083-71eb5e27e6ac"),
        "DailyChallengeDialog" | "SnoutCoinShop" => Some("ef7ae3f3-3a36-4b57-b209-f630d4837795"),
        "LeaderboardDialog" | "SeasonEndDialog" => Some("c41d8f89-5141-453b-8bdb-e42dde37860e"),
        "LeaderboardEntry" | "SingleLeaderboardEntry" => {
            Some("1d802ff7-c5a1-45ef-a084-97f81f37f0c8")
        }
        "LevelRowUnlockPanel" => Some("3f47c76b-3891-4685-adae-029a5e655dc5"),
        "PurchasePiggyPackIAP" | "WatchSnoutCoinAd" => Some("ab0c6536-dfc1-46a1-8276-59280b355188"),
        "ResourceBar" => Some("eea6164b-a556-4787-9420-d82b390e6675"),
        "ScrapButton" => Some("913d3f55-e5fe-49f7-b072-ad18875d9ce0"),
        "SnoutButton" => Some("dfb4e969-93e2-4d7d-969b-29732cc266c7"),
        "WorkshopIntroduction" => Some("f4bb39c9-0562-4c34-bc01-b66ce7c4edc2"),
        _ => None,
    }
}

pub(super) fn atlas_for_material_guid(material_guid: &str) -> Option<&'static str> {
    let prefix = material_guid.get(..8).unwrap_or(material_guid);
    match prefix {
        "ce5a9931" | "d645821c" | "125eb5b4" | "0e790fab" | "353dd850" => Some("IngameAtlas.png"),
        "211b2b9c" | "aca6a4c6" | "765e60c2" | "4ab535f3" | "4eeb62bc" => Some("IngameAtlas2.png"),
        "2a21c011" | "ad767d84" | "7192b13e" | "a6f51d97" | "7975d66d" => Some("IngameAtlas3.png"),
        _ => None,
    }
}
