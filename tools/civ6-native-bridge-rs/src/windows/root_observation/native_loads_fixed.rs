//! Complete pinned load bodies and all chained unwind partitions.
use super::*;
pub(super) struct Fragment {
    pub begin: u32,
    pub end: u32,
    pub unwind: u32,
    pub hex: &'static str,
}
pub(super) struct FixedFrame {
    pub begin: u32,
    pub end: u32,
    pub sha: &'static str,
    pub parts: &'static [Fragment],
}
pub(super) const FRAMES:&[FixedFrame]=&[
FixedFrame{begin:0x273f40,end:0x2741f2,sha:"9d54b5a08c28061169bee30ad6211acf9ec984d8477a2f1e7532493c287e4c31",parts:&[
Fragment{begin:0x273f40,end:0x2741f2,unwind:0x1177ffc,hex:"012a0b002ae423042a7422042a6421042a3420042a011e0415500000"},
]},
FixedFrame{begin:0x16b5c0,end:0x16bd86,sha:"50e27fc98d68bfe2b251fd78c48d7293e235bc3e4f5bfae6650a149916b2a816",parts:&[
Fragment{begin:0x16b5c0,end:0x16b5fb,unwind:0x11682b0,hex:"01220700220158040df00be009d0073006500000"},
Fragment{begin:0x16b5fb,end:0x16b809,unwind:0x11682c4,hex:"2122060022c4600415745f0408645e04c0b51600fbb51600b0821601"},
Fragment{begin:0x16b809,end:0x16bd74,unwind:0x11682e0,hex:"2100060000c4600400745f0400645e04c0b51600fbb51600b0821601"},
Fragment{begin:0x16bd74,end:0x16bd86,unwind:0x11682fc,hex:"21000000c0b51600fbb51600b0821601"},
]},
FixedFrame{begin:0x166b40,end:0x166e6e,sha:"1e0398e0d647b4fe62209de233696866e3b4100c7bb2076ba9bbf171b55c10ac",parts:&[
Fragment{begin:0x166b40,end:0x166d04,unwind:0x1168528,hex:"0141080041341b041b01150406f0047003600250"},
Fragment{begin:0x166d04,end:0x166de2,unwind:0x116853c,hex:"2108020008e41404406b1600046d160028851601"},
Fragment{begin:0x166de2,end:0x166df7,unwind:0x1168564,hex:"21000000406b1600046d160028851601"},
Fragment{begin:0x166df7,end:0x166e41,unwind:0x1168550,hex:"2100020000e41404406b1600046d160028851601"},
Fragment{begin:0x166e41,end:0x166e6e,unwind:0x1168564,hex:"21000000406b1600046d160028851601"},
]},
FixedFrame{begin:0xef220,end:0xef27e,sha:"d64739da3a1a332aa8f7c9f6262a31d9e5832931b15d32e71da7df4a2c9ca4ff",parts:&[
Fragment{begin:0xef220,end:0xef27e,unwind:0x1164b0c,hex:"010a04000a340a000a720670"},
]},
FixedFrame{begin:0x82e50,end:0x83151,sha:"06e5b600274af1a5aa9edd0ff21b9b77ba952fcc5f37f0256b6045583177425a",parts:&[
Fragment{begin:0x82e50,end:0x82e6b,unwind:0x11608e0,hex:"011b05001b01140406e004c002500000"},
Fragment{begin:0x82e6b,end:0x82f43,unwind:0x11608f0,hex:"2129060029f412041a74130408341a04502e08006b2e0800e0081601"},
Fragment{begin:0x82f43,end:0x83005,unwind:0x116090c,hex:"2108020008641b046b2e0800432f0800f0081601"},
Fragment{begin:0x83005,end:0x83110,unwind:0x1160920,hex:"210000006b2e0800432f0800f0081601"},
Fragment{begin:0x83110,end:0x83151,unwind:0x1160930,hex:"21000000502e08006b2e0800e0081601"},
]},
FixedFrame{begin:0x82da0,end:0x82e4a,sha:"f378725f5d66de86e8a51c9bbd7f15ed4cb780881d972844545d4d83775cf6f7",parts:&[
Fragment{begin:0x82da0,end:0x82e4a,unwind:0x11f0fac,hex:"010a04000a3406000a320670"},
]},
FixedFrame{begin:0x162480,end:0x1624fd,sha:"08144b34d984eb6e4155c17c25e48d1573a37b3d8634ada573649e300f2b2b74",parts:&[]},
FixedFrame{begin:0x6b7d0,end:0x6b7df,sha:"8abb8cabbfea3c6a2d54bc775d142185b51e42650ac812c233ddb081782b8941",parts:&[]},
FixedFrame{begin:0x16d280,end:0x16d2ac,sha:"4ff3227c8aabaec19fa769b341eb45536b383ee0d85beb37d64a043c2fb952e7",parts:&[]},
FixedFrame{begin:0x986150,end:0x986190,sha:"c84d8c0a0b31a26878900ca07cc1d6e568eda56b5ab5af8ae7b3148d2d3c5e90",parts:&[
Fragment{begin:0x986150,end:0x986190,unwind:0x11bc894,hex:"0104010004620000"},
]},
FixedFrame{begin:0x9858b0,end:0x985967,sha:"f2ec497b5b9574bb8c9fb9bc2c95bc8a8a6508fcdfe6c7301006d07ebd669a96",parts:&[
Fragment{begin:0x9858b0,end:0x985967,unwind:0x11db100,hex:"010f06000f640a000f3409000f520b70"},
]},
FixedFrame{begin:0x985b50,end:0x985b72,sha:"360648e5cf675111d5dad3e0554a6e10c3070fb8e9b6db29dc57f60a3cf10da2",parts:&[
Fragment{begin:0x985b50,end:0x985b72,unwind:0x11c0b10,hex:"0106020006320230"},
]},
FixedFrame{begin:0x83450,end:0x8345b,sha:"d27ddb8cec7a9454c887e97241eb79075b58bd611338dc64211f263dc2307755",parts:&[]},
FixedFrame{begin:0xebc20,end:0xebcd8,sha:"9859e0d60da0c783ad5b9ed0a058f335b3080f9e9ebd8655936a992f9abfb254",parts:&[
Fragment{begin:0xebc20,end:0xebcd8,unwind:0x1198528,hex:"0106020006520230"},
]},
FixedFrame{begin:0xdb810,end:0xdb989,sha:"971f1af6bacfde1ecaefe257e322c95c9157b4fec318adef3918df605891ae92",parts:&[
Fragment{begin:0xdb810,end:0xdb81c,unwind:0x11c3164,hex:"010804000872047003600230"},
Fragment{begin:0xdb81c,end:0xdb852,unwind:0x1164aa8,hex:"2112060012f40f000ac40d0005540c0010b80d001cb80d0064311c01"},
Fragment{begin:0xdb852,end:0xdb8ae,unwind:0x1164ac4,hex:"2105020005e40e001cb80d0052b80d00a84a1601"},
Fragment{begin:0xdb8ae,end:0xdb8c2,unwind:0x1164ad8,hex:"210000001cb80d0052b80d00a84a1601"},
Fragment{begin:0xdb8c2,end:0xdb8de,unwind:0x1164ae8,hex:"2100020000f40f0010b80d001cb80d0064311c01"},
Fragment{begin:0xdb8de,end:0xdb989,unwind:0x1164afc,hex:"2100000010b80d001cb80d0064311c01"},
]},
FixedFrame{begin:0x63f60,end:0x64008,sha:"78cf5bfa60810508895774d4a931f1b315c98e766541bc1569230e285b72989a",parts:&[
Fragment{begin:0x63f60,end:0x64008,unwind:0x1164b0c,hex:"010a04000a340a000a720670"},
]},
FixedFrame{begin:0x5f470,end:0x5f4d2,sha:"f08ec6b2beda8f5e3f8eb976bfe1fd12eb1c09143e8083a2cf398afd53bc7027",parts:&[
Fragment{begin:0x5f470,end:0x5f47f,unwind:0x1237a38,hex:"0106020006320260"},
Fragment{begin:0x5f47f,end:0x5f4cc,unwind:0x115f9ec,hex:"21110400117407000534060070f405007ff40500387a2301"},
Fragment{begin:0x5f4cc,end:0x5f4d2,unwind:0x115fa04,hex:"2100000070f405007ff40500387a2301"},
]},
FixedFrame{begin:0x16c920,end:0x16c93d,sha:"62ffdff96880a00ccf1b2f9cc5e232f53baad1b3419ae8ae429606a7ea4e3d7f",parts:&[
Fragment{begin:0x16c920,end:0x16c93d,unwind:0x11bc894,hex:"0104010004620000"},
]},
FixedFrame{begin:0x165820,end:0x165bd4,sha:"fb5156b91d21b7dec65b2c6cbda213556169b94ee845f89e71b830ab8c75a28b",parts:&[
Fragment{begin:0x165820,end:0x1658ac,unwind:0x116882c,hex:"01740a0074e412046c741304646414045c3418040d011504"},
Fragment{begin:0x1658ac,end:0x165a8f,unwind:0x1168844,hex:"210802000854190420581600ac5816002c881601"},
Fragment{begin:0x165a8f,end:0x165bd4,unwind:0x1168858,hex:"2100000020581600ac5816002c881601"},
]},
FixedFrame{begin:0x163cc0,end:0x1640f8,sha:"48bfa3a88d7275e5d3da891e91e226c3ae2abd9d1ca414c0b957a4b187533d94",parts:&[
Fragment{begin:0x163cc0,end:0x163ce9,unwind:0x116830c,hex:"011004001001170403600250"},
Fragment{begin:0x163ce9,end:0x163e33,unwind:0x1168318,hex:"2110040010e4130408341c04c03c1600e93c16000c831601"},
Fragment{begin:0x163e33,end:0x163f0b,unwind:0x1168330,hex:"2120040020f4120408c41504e93c1600333e160018831601"},
Fragment{begin:0x163f0b,end:0x16400f,unwind:0x1168348,hex:"2110040010d4140408741604333e16000b3f160030831601"},
Fragment{begin:0x16400f,end:0x16401f,unwind:0x1168360,hex:"21000000333e16000b3f160030831601"},
Fragment{begin:0x16401f,end:0x16403b,unwind:0x1168370,hex:"21000000e93c1600333e160018831601"},
Fragment{begin:0x16403b,end:0x16409f,unwind:0x1168380,hex:"21000c0000f4120400e4130400d4140400c415040074160400341c04c03c1600e93c16000c831601"},
Fragment{begin:0x16409f,end:0x1640ec,unwind:0x11683a8,hex:"2100080000f4120400e4130400c4150400341c04c03c1600e93c16000c831601"},
Fragment{begin:0x1640ec,end:0x1640f8,unwind:0x11683c8,hex:"21000000c03c1600e93c16000c831601"},
]},
];
pub(super) unsafe fn verify(module: usize) -> Result<(), &'static str> {
    for f in FRAMES {
        let len = (f.end - f.begin) as usize;
        if !committed_range(module + f.begin as usize, len, 1, false) {
            return Err("load body range");
        }
        let bytes =
            unsafe { std::slice::from_raw_parts((module + f.begin as usize) as *const u8, len) };
        if format!("{:x}", Sha256::digest(bytes)) != f.sha {
            return Err("load complete body differs");
        }
        for p in f.parts {
            let mut base = 0;
            let row = unsafe {
                windows_sys::Win32::System::Diagnostics::Debug::RtlLookupFunctionEntry(
                    module as u64 + u64::from(p.begin),
                    &mut base,
                    null_mut(),
                )
            };
            let expected = [
                p.begin.to_le_bytes(),
                p.end.to_le_bytes(),
                p.unwind.to_le_bytes(),
            ]
            .concat();
            if base != module as u64
                || copy_current::<12>(row as u64)
                    .as_ref()
                    .map(|v| v.as_slice())
                    != Some(expected.as_slice())
            {
                return Err("load pdata differs");
            }
            for (i, byte) in (0..p.hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&p.hex[i..i + 2], 16).unwrap())
                .enumerate()
            {
                if copy_current::<1>(module as u64 + u64::from(p.unwind) + i as u64) != Some([byte])
                {
                    return Err("load unwind differs");
                }
            }
        }
    }
    Ok(())
}
