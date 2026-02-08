// Tor v3 onion seeds
pub const TOR_SEEDS: &[&str] = &[
    "http://zqktlwiuavvvqqt4ybvgvi7tyo4hjl5xgfuvpdf6otjiycgwqbym2qad.onion/wiki/index.php/Main_Page",
    "http://ciadotgov4sjwlzihbbgxnqg3xiyrg7so2r2o3lt5wz5ypk4sxyjstad.onion/",
    "http://dreadytofatroptsdj6io7l3xptbet6onoyno2yv7jicoxknyazubrad.onion/",
    "http://s4k4ceiapwwgcm3mkb6e4diqecpo7kvdnfr5gg7sph7jjppqkvwwqtyd.onion/",
    "http://torchdeedp3i2jigzjdmfpn5ttjhthh5wbmda2rr3jvqjg5p77c54dqd.onion/",
    "http://juhanurmihxlp77nkq76byazcldy2hlmovfu2epvl5ankdibsot4csyd.onion/",
    "http://xmh57jrknzkhv6y3ls3ubitzfqnkrwxhopf5aygthi7d6rplyvk3noyd.onion/",
    "http://tor66sewebgixwhcqfnp5inzp5x5uohhdy3kvtnyfxc2e5mxiuh34iid.onion/",
];

// I2P eepsite seeds
pub const I2P_SEEDS: &[&str] = &[
    // Directories & registries
    "http://identiguy.i2p/",
    "http://notbob.i2p/",
    "http://reg.i2p/",
    "http://inr.i2p/",
    "http://stats.i2p/",
    "http://i2pjump.i2p/",
    "http://linkz.i2p/",
    // Search engines
    "http://eepsites.i2p/",
    "http://legwork.i2p/",
    "http://i2psearch.i2p/",
    "http://i2pfind.i2p/",
    // Forums & wikis
    "http://forum.i2p/",
    "http://zzz.i2p/",
    "http://i2pforum.i2p/",
    "http://ramble.i2p/",
    "http://ugha.i2p/",
    // Community & aggregators
    "http://planet.i2p/",
    "http://www.i2p2.i2p/",
    "http://echelon.i2p/",
    "http://plugins.i2p/",
    "http://tracker2.postman.i2p/",
];

// ZeroNet zite seeds
pub const ZERONET_SEEDS: &[&str] = &[
    // Directories and indexes (most important for discovery)
    "http://1HeLLo4uzjaLetFx6NH3PMwFP3qbRbTf3D.bit/", // ZeroHello
    "http://1SitesVCdgNfHojzf2aGKQrD4dteAZR1k.bit/",  // ZeroSitesX (index)
    "http://1SiTEs2D3rCBxeMoLHXei2UYqFcxctdwB.bit/",  // ZeroSites (index)
    "http://1Dt7FR5aNLkqAjmosWh8cMWzJu633GYN6u.bit/", // ZeroCentral
    "http://1Mr5rX9TauvaGReB4RjCaE6D37FJQaY5Ba.bit/", // Kaffiene Search
    "http://13EYKqmPpwzBU4iaQq9Y4vfVMgj8dHeLkc.bit/", // Zoogle Zearch
    "http://1JBFNPrAGp1nQX6RsAN6oRqCfvtoeWoion.bit/", // Dream Search
    "http://15Pf9VVuDT8NSWj1qUBh4V89yPmrmzRw6a.bit/", // Important Zites
    // Community & social
    "http://1BLogC9LN4oPDcruNz3qo1ysa133E9AGg8.bit/", // ZeroBlog
    "http://1TaLkFrMwvbNsooF4ioKAY9EuxTBTjipT.bit/",  // ZeroTalk
    "http://1EgyL4nj9DmeSSQg3fytxGJjihxtmMon5y.bit/", // ZeroTalk++
    "http://15UYrA7aXr2Nto1Gg4yWXpY3EAJwafMTNk.bit/", // ThreadIt
    "http://1CWkZv7fQAKxTVjZVrLZ8VHcrN6YGGcdky.bit/", // ThunderWave (chat)
    "http://1HMLvnRWViMnuvZc5LK4Dm86sZNcSH1jdh.bit/", // UnlimitTalk
    "http://1LfvE91ZF18jdG3wW62Dw7NtfTZh737KPL.bit/", // NetTalk
    // Social media
    "http://1MeFqFfFFGQfa1J3gJyYYUvb5Lksczq7nH.bit/", // ZeroMe
    "http://1JgcgZQ5a2Gxc4Cfy32szBJC68mMGusBjC.bit/", // XeroMe
    "http://1GLndW2MJn7japuF3X2tbfBqgPMR52zaLQ.bit/", // 0Hub
    // Mail
    "http://1MaiL5gfBM1cyb4a8e3iiL8L5gXmoAJu27.bit/", // ZeroMail
    "http://1MaiLX6j5MSddyu8oh5CxxGrhMcSmRo6N8.bit/", // ZeroMailX
    // Developer tools
    "http://1GitLiXB6t5r8vuU2zC6a8GYj9ME6HMQ4t.bit/", // Git Center
    "http://18ryVioVmwFYzhRZKTjKqGYCjkUjoxH3k6.bit/", // ZeroNet DevZone
    // File sharing & torrents
    "http://192dZ1EG5tU7PnCfuwGMDEBrr2eLqvs4t3.bit/", // ZeroTorrent
    "http://1UPLoADsqDWzMEyqLNin8GPcWoqiihu1g.bit/",  // StoreAge
    "http://1DYwX4W1qfSaiihfN5gW6ahLp5332TawH5.bit/", // ZeroPub (books)
    // Paste & utilities
    "http://1MgHVPCE1ve6QfKrgsqCURzRj72HrRWioz.bit/", // NullPaste
    "http://1GNTAKCimBv5xEnt7QvkDn8sTkEPj7ZYTL.bit/", // ZeroShortener
];

// Freenet/Hyphanet freesite seeds
pub const FREENET_SEEDS: &[&str] = &[
    "freenet:USK@QRZAI1nSm~dAY2hTgiW1-C7P3CEGV9tqKaSOSmLiDMI,DcnBm6MV9zPKaT3MHRmBisfzI8tDhjVBNyjaHIJx5Oo,AQACAAE/activelink-index/0/",
    "freenet:USK@pXTEkrmFi9FIhv3EAf8p4MRPKHmn4U-Y2LUsPfEvbdg,SDnSl03Gbl1r1oKz6fy4qj35gEIieGS3lYq8K5MFb5Q,AQACAAE/index/0/",
    "freenet:USK@5ijbfKSJ4kPZTRDzq363CHteEUiSZjrO-E36vbHvnIU,ZEZqPXeuYiyokY2r0wkhJr5cy7KBH9omkuWDqSC6PLs,AQACAAE/clean-spider/37/",
];

// Lokinet SNApp seeds
pub const LOKINET_SEEDS: &[&str] = &[
    // Directories and diverse content (prioritize for discovery)
    "http://lokinet.wiki/",
    "http://minecraft.loki/",
    "http://chiefsnapp.loki/",
    "http://invidious.loki/",
    "http://dw68y1xhptqbhcm5s8aaaip6dbopykagig5q5u1za4c7pzxto77y.loki/wiki/",
    "http://dw68y1xhptqbhcm5s8aaaip6dbopykagig5q5u1za4c7pzxto77y.loki/",
    "http://kqrtg5wz4qbyjprujkz33gza7r73iw3ainqp1mz5zmu16symcdwy.loki/",
    // Blockchain explorers (will be limited by penalty system after 100 pages)
    "http://kcpyawm9se7trdbzncimdi5t7st4p5mh9i1mg7gkpuubi4k4ku1y.loki/",
    "http://blocks.loki/",
];

/// Combined default seeds - merges all network seeds
pub fn get_all_seeds() -> Vec<&'static str> {
    let mut seeds = Vec::new();
    seeds.extend_from_slice(TOR_SEEDS);
    seeds.extend_from_slice(I2P_SEEDS);
    seeds.extend_from_slice(ZERONET_SEEDS);
    seeds.extend_from_slice(FREENET_SEEDS);
    seeds.extend_from_slice(LOKINET_SEEDS);
    seeds
}

/// Check if an .onion host is a valid v3 address (56 base32 chars).
pub fn is_v3_onion(host: &str) -> bool {
    let Some(name) = host.strip_suffix(".onion") else {
        return false;
    };
    name.len() == 56
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || ('2'..='7').contains(&c))
}
