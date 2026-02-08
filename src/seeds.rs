/// Default seed URLs for all supported dark web networks
pub const DEFAULT_SEEDS: &[&str] = &[
    // Tor v3 onion seeds
    "http://zqktlwiuavvvqqt4ybvgvi7tyo4hjl5xgfuvpdf6otjiycgwqbym2qad.onion/wiki/index.php/Main_Page",
    "http://ciadotgov4sjwlzihbbgxnqg3xiyrg7so2r2o3lt5wz5ypk4sxyjstad.onion/",
    "http://dreadytofatroptsdj6io7l3xptbet6onoyno2yv7jicoxknyazubrad.onion/",
    "http://s4k4ceiapwwgcm3mkb6e4diqecpo7kvdnfr5gg7sph7jjppqkvwwqtyd.onion/",
    "http://torchdeedp3i2jigzjdmfpn5ttjhthh5wbmda2rr3jvqjg5p77c54dqd.onion/",
    "http://juhanurmihxlp77nkq76byazcldy2hlmovfu2epvl5ankdibsot4csyd.onion/",
    "http://xmh57jrknzkhv6y3ls3ubitzfqnkrwxhopf5aygthi7d6rplyvk3noyd.onion/",
    "http://tor66sewebgixwhcqfnp5inzp5x5uohhdy3kvtnyfxc2e5mxiuh34iid.onion/",
    // I2P seeds — directories & registries
    "http://identiguy.i2p/",
    "http://notbob.i2p/",
    "http://reg.i2p/",
    "http://inr.i2p/",
    "http://stats.i2p/",
    "http://i2pjump.i2p/",
    "http://linkz.i2p/",
    // I2P seeds — search engines
    "http://eepsites.i2p/",
    "http://legwork.i2p/",
    "http://i2psearch.i2p/",
    "http://i2pfind.i2p/",
    // I2P seeds — forums & wikis
    "http://forum.i2p/",
    "http://zzz.i2p/",
    "http://i2pforum.i2p/",
    "http://ramble.i2p/",
    "http://ugha.i2p/",
    // I2P seeds — community & aggregators
    "http://planet.i2p/",
    "http://www.i2p2.i2p/",
    "http://echelon.i2p/",
    "http://plugins.i2p/",
    "http://tracker2.postman.i2p/",
    // ZeroNet seeds (.bit) — directories & communities
    "http://zerosites.bit/",
    "http://talk.bit/",
    "http://zerowiki.bit/",
    "http://zeroboard.bit/",
    "http://kaffiene.bit/",
    "http://zerosearch.bit/",
    "http://0list.bit/",
    "http://ThreadIt.bit/",
    "http://blog.zeronetwork.bit/",
    "http://mail.zeronetwork.bit/",
    "http://talk.zeronetwork.bit/",
    "http://me.zeronetwork.bit/",
    "http://millchan.bit/",
    "http://0net.bit/",
    "http://zeroname.bit/",
    "http://zerolstn.bit/",
    "http://kxovq.bit/",
    // Freenet/Hyphanet seeds — well-known freesites (USK keys)
    "freenet:USK@QRZAI1nSm~dAY2hTgiW1-C7P3CEGV9tqKaSOSmLiDMI,DcnBm6MV9zPKaT3MHRmBisfzI8tDhjVBNyjaHIJx5Oo,AQACAAE/activelink-index/0/",
    "freenet:USK@pXTEkrmFi9FIhv3EAf8p4MRPKHmn4U-Y2LUsPfEvbdg,SDnSl03Gbl1r1oKz6fy4qj35gEIieGS3lYq8K5MFb5Q,AQACAAE/index/0/",
    // Lokinet seeds — .loki SNApps (base32 addresses + ONS names)
    "http://dw68y1xhptqbhcm5s8aaaip6dbopykagig5q5u1za4c7pzxto77y.loki/wiki/",
    "http://dw68y1xhptqbhcm5s8aaaip6dbopykagig5q5u1za4c7pzxto77y.loki/",
    "http://kcpyawm9se7trdbzncimdi5t7st4p5mh9i1mg7gkpuubi4k4ku1y.loki/",
    "http://f59q8xa7m3bmkfi11oijuh4w77ar49hgx8pdexoqonehcywnifsy.loki/",
    "http://arweave.f59q8xa7m3bmkfi11oijuh4w77ar49hgx8pdexoqonehcywnifsy.loki/",
    "http://git.f59q8xa7m3bmkfi11oijuh4w77ar49hgx8pdexoqonehcywnifsy.loki/",
    "http://inqomcym4rhouwhxhnwcczj3j8ykb4ehxkh3yfqtpjybatdettuo.loki/",
    "http://exit.loki/",
    "http://directory.loki/",
    "http://probably.loki/",
    // Additional diversity seeds from 2026 Lokinet directory
    "http://pomf.dw68y1xhptqbhcm5s8aaaip6dbopykagig5q5u1za4c7pzxto77y.loki/",
    "http://ikhuwze4r597pn6shynuc7dk1io1ahywxn5e1wu769apzfmcyjfo.loki/",
    "http://8bb19w1gugu7yq3cyck63gbo18udodab1b6zr1uykdphm37ushco.loki/",
];

/// Check if an .onion host is a valid v3 address (56 base32 chars).
pub fn is_v3_onion(host: &str) -> bool {
    let Some(name) = host.strip_suffix(".onion") else {
        return false;
    };
    name.len() == 56 && name.chars().all(|c| c.is_ascii_lowercase() || ('2'..='7').contains(&c))
}
