//! Calibrated ceilings for the immutable original Window8 Source closure.
//! These are measured fixed-profile ceilings, not a generic analytical AST bound.
//! Exact artifact digests are checked without allocating before any preparation.
use crate::parser_session_pure_source::{
    PreparedParserPureSource, PureSourceLimits, PureSourceRefusal, PureSourceReservation,
};
use crate::parser_session_window8_ports::PORTS;
use sha2::{Digest, Sha256};
struct Calibration {
    name: &'static str,
    digest: [u8; 32],
    programs: usize,
    program: usize,
    decode: usize,
    ty: usize,
    preparation: usize,
    retained: usize,
}
const CALIBRATION: [Calibration; 54] = [
    Calibration {
        name: "language-proposal-window8-feature-context",
        digest: [
            200, 242, 105, 123, 63, 151, 149, 195, 77, 115, 255, 125, 174, 0, 147, 70, 196, 185,
            214, 148, 234, 232, 249, 49, 190, 212, 155, 203, 132, 250, 196, 150,
        ],
        programs: 1,
        program: 633974,
        decode: 9264062,
        ty: 13142,
        preparation: 10065390,
        retained: 1104264,
    },
    Calibration {
        name: "language-proposal-window8-origins",
        digest: [
            88, 41, 255, 15, 192, 69, 53, 155, 249, 73, 57, 166, 90, 47, 32, 95, 38, 197, 119, 51,
            73, 160, 49, 122, 53, 149, 127, 18, 102, 212, 13, 23,
        ],
        programs: 1,
        program: 872606,
        decode: 11908613,
        ty: 17524,
        preparation: 27331270,
        retained: 16323403,
    },
    Calibration {
        name: "language-proposal-window8-v2-feature-values",
        digest: [
            11, 172, 179, 171, 29, 132, 210, 232, 189, 89, 19, 37, 155, 255, 188, 126, 251, 222,
            230, 123, 95, 218, 45, 178, 17, 136, 159, 102, 199, 242, 32, 165,
        ],
        programs: 1,
        program: 2710161,
        decode: 41942420,
        ty: 13142,
        preparation: 42904976,
        retained: 1294590,
    },
    Calibration {
        name: "language-window8-available",
        digest: [
            243, 71, 161, 67, 23, 172, 206, 193, 2, 112, 171, 144, 94, 33, 12, 57, 255, 107, 210,
            165, 195, 188, 86, 181, 52, 180, 39, 172, 13, 224, 141, 74,
        ],
        programs: 1,
        program: 58280,
        decode: 720989,
        ty: 6692,
        preparation: 3982285,
        retained: 3252017,
    },
    Calibration {
        name: "language-window8-choice-frontier",
        digest: [
            248, 234, 126, 107, 119, 216, 215, 39, 125, 237, 30, 137, 81, 252, 67, 66, 25, 11, 10,
            24, 243, 216, 255, 115, 111, 103, 220, 133, 137, 125, 223, 183,
        ],
        programs: 1,
        program: 144609,
        decode: 2142473,
        ty: 12212,
        preparation: 2213601,
        retained: 27890,
    },
    Calibration {
        name: "language-window8-class-context",
        digest: [
            163, 238, 238, 175, 191, 6, 115, 112, 204, 48, 69, 128, 107, 68, 161, 183, 205, 40, 40,
            14, 115, 9, 34, 39, 29, 49, 237, 99, 98, 74, 4, 214,
        ],
        programs: 1,
        program: 239825,
        decode: 3571092,
        ty: 14585,
        preparation: 5145940,
        retained: 1556511,
    },
    Calibration {
        name: "language-window8-class-index",
        digest: [
            97, 180, 83, 155, 85, 89, 235, 58, 45, 124, 48, 8, 30, 95, 178, 178, 13, 26, 204, 143,
            109, 84, 55, 109, 30, 175, 244, 75, 181, 165, 166, 42,
        ],
        programs: 1,
        program: 19564,
        decode: 248399,
        ty: 1603,
        preparation: 504971,
        retained: 249669,
    },
    Calibration {
        name: "language-window8-class-relation",
        digest: [
            113, 216, 149, 89, 125, 246, 2, 18, 192, 148, 50, 41, 51, 41, 224, 183, 225, 199, 125,
            122, 18, 114, 153, 63, 37, 5, 11, 98, 66, 124, 199, 19,
        ],
        programs: 1,
        program: 26903,
        decode: 369875,
        ty: 2748,
        preparation: 634230,
        retained: 246482,
    },
    Calibration {
        name: "language-window8-class-relations",
        digest: [
            225, 22, 133, 229, 209, 161, 107, 2, 35, 245, 154, 226, 208, 184, 160, 22, 34, 105,
            132, 176, 59, 93, 63, 59, 141, 101, 175, 13, 25, 171, 243, 164,
        ],
        programs: 1,
        program: 49089,
        decode: 657501,
        ty: 2748,
        preparation: 1278353,
        retained: 553251,
    },
    Calibration {
        name: "language-window8-complete",
        digest: [
            11, 174, 217, 91, 202, 72, 119, 210, 234, 157, 240, 6, 132, 116, 79, 89, 39, 95, 118,
            253, 19, 53, 140, 66, 119, 151, 248, 105, 29, 146, 87, 40,
        ],
        programs: 1,
        program: 217492,
        decode: 3020123,
        ty: 10726,
        preparation: 3106646,
        retained: 32621,
    },
    Calibration {
        name: "language-window8-dependency-compatible",
        digest: [
            3, 218, 3, 15, 243, 71, 231, 73, 211, 95, 246, 221, 58, 207, 211, 197, 181, 61, 174, 3,
            109, 146, 37, 149, 186, 85, 53, 184, 112, 115, 223, 196,
        ],
        programs: 34,
        program: 1470310,
        decode: 22405264,
        ty: 31982,
        preparation: 24252529,
        retained: 1990112,
    },
    Calibration {
        name: "language-window8-empty-codes",
        digest: [
            208, 184, 2, 124, 125, 222, 175, 40, 245, 250, 70, 69, 173, 35, 252, 139, 119, 190,
            102, 88, 129, 229, 68, 45, 162, 102, 32, 162, 93, 21, 147, 223,
        ],
        programs: 1,
        program: 982,
        decode: 7553,
        ty: 182,
        preparation: 18596,
        retained: 4366,
    },
    Calibration {
        name: "language-window8-empty-grow",
        digest: [
            45, 224, 100, 80, 62, 53, 189, 192, 255, 86, 177, 215, 69, 99, 178, 74, 136, 100, 187,
            31, 28, 225, 153, 240, 202, 169, 89, 228, 7, 253, 24, 96,
        ],
        programs: 1,
        program: 958194,
        decode: 13932730,
        ty: 29498,
        preparation: 16316529,
        retained: 2689352,
    },
    Calibration {
        name: "language-window8-feature-context",
        digest: [
            181, 48, 102, 33, 250, 191, 60, 157, 227, 185, 121, 192, 72, 154, 124, 70, 216, 137,
            190, 85, 190, 32, 133, 81, 137, 27, 91, 170, 111, 184, 165, 55,
        ],
        programs: 1,
        program: 400806,
        decode: 5713874,
        ty: 12398,
        preparation: 6505359,
        retained: 1073124,
    },
    Calibration {
        name: "language-window8-feature-values",
        digest: [
            152, 0, 202, 14, 206, 157, 29, 211, 65, 3, 13, 65, 242, 252, 105, 78, 104, 36, 219,
            141, 114, 33, 235, 179, 2, 153, 224, 136, 23, 226, 30, 79,
        ],
        programs: 1,
        program: 2108006,
        decode: 31568583,
        ty: 12398,
        preparation: 32503504,
        retained: 1113738,
    },
    Calibration {
        name: "language-window8-grow",
        digest: [
            233, 45, 223, 9, 63, 102, 16, 73, 180, 36, 68, 153, 0, 15, 235, 82, 238, 77, 114, 48,
            226, 108, 245, 215, 93, 12, 34, 83, 118, 0, 93, 127,
        ],
        programs: 1,
        program: 1357438,
        decode: 20777242,
        ty: 29948,
        preparation: 23164439,
        retained: 2699722,
    },
    Calibration {
        name: "language-window8-independent-commit-initialize",
        digest: [
            12, 174, 54, 144, 241, 163, 133, 20, 132, 217, 96, 55, 234, 141, 134, 131, 175, 90,
            184, 38, 72, 212, 85, 146, 249, 99, 151, 2, 82, 164, 95, 95,
        ],
        programs: 1,
        program: 130909,
        decode: 1547640,
        ty: 20483,
        preparation: 12145579,
        retained: 10515386,
    },
    Calibration {
        name: "language-window8-independent-commit-rebase",
        digest: [
            198, 186, 183, 42, 234, 121, 125, 102, 234, 147, 88, 11, 27, 79, 137, 30, 86, 19, 235,
            161, 185, 169, 124, 190, 74, 252, 136, 11, 111, 16, 132, 230,
        ],
        programs: 1,
        program: 130827,
        decode: 1715811,
        ty: 44898,
        preparation: 2541251,
        retained: 610291,
    },
    Calibration {
        name: "language-window8-initialize",
        digest: [
            192, 195, 200, 8, 229, 188, 128, 255, 98, 59, 51, 209, 58, 102, 216, 212, 157, 51, 93,
            106, 126, 201, 96, 248, 226, 144, 58, 23, 1, 160, 36, 124,
        ],
        programs: 1,
        program: 53900,
        decode: 699962,
        ty: 10726,
        preparation: 2619064,
        retained: 1880197,
    },
    Calibration {
        name: "language-window8-move-apply",
        digest: [
            82, 57, 180, 87, 142, 19, 139, 36, 240, 107, 27, 144, 224, 222, 205, 222, 140, 48, 85,
            250, 104, 236, 130, 37, 218, 243, 61, 83, 249, 185, 224, 13,
        ],
        programs: 1,
        program: 3186685,
        decode: 45665305,
        ty: 12886,
        preparation: 51005572,
        retained: 6581958,
    },
    Calibration {
        name: "language-window8-move-context",
        digest: [
            43, 12, 91, 47, 55, 233, 67, 76, 41, 110, 182, 24, 102, 89, 115, 157, 185, 110, 240,
            63, 97, 244, 145, 53, 99, 17, 72, 22, 151, 120, 185, 129,
        ],
        programs: 1,
        program: 735465,
        decode: 10897137,
        ty: 13323,
        preparation: 12484285,
        retained: 1605752,
    },
    Calibration {
        name: "language-window8-move-legal-left",
        digest: [
            36, 39, 146, 26, 145, 237, 248, 3, 25, 101, 106, 37, 100, 196, 27, 157, 98, 236, 55,
            214, 173, 205, 112, 92, 249, 141, 198, 141, 141, 115, 218, 227,
        ],
        programs: 1,
        program: 467698,
        decode: 6700503,
        ty: 12886,
        preparation: 8292028,
        retained: 1590054,
    },
    Calibration {
        name: "language-window8-move-legal-reduce",
        digest: [
            34, 148, 22, 35, 61, 128, 211, 108, 50, 54, 73, 40, 150, 7, 165, 122, 233, 20, 2, 149,
            238, 126, 27, 79, 24, 152, 218, 218, 141, 254, 114, 176,
        ],
        programs: 1,
        program: 368289,
        decode: 5277836,
        ty: 12886,
        preparation: 6857470,
        retained: 1571111,
    },
    Calibration {
        name: "language-window8-move-legal-right-nonroot",
        digest: [
            12, 109, 223, 124, 122, 98, 105, 136, 16, 226, 58, 26, 191, 0, 228, 166, 195, 84, 95,
            115, 166, 61, 94, 119, 248, 10, 34, 73, 248, 28, 224, 227,
        ],
        programs: 1,
        program: 512969,
        decode: 7339582,
        ty: 12886,
        preparation: 8932597,
        retained: 1590616,
    },
    Calibration {
        name: "language-window8-move-legal-right-root",
        digest: [
            17, 255, 41, 177, 230, 123, 91, 18, 56, 80, 226, 102, 205, 93, 95, 38, 250, 99, 121,
            50, 170, 18, 253, 104, 58, 174, 125, 126, 32, 183, 93, 238,
        ],
        programs: 1,
        program: 525960,
        decode: 7527846,
        ty: 12886,
        preparation: 9122156,
        retained: 1591059,
    },
    Calibration {
        name: "language-window8-move-legal-shift",
        digest: [
            232, 135, 207, 54, 65, 24, 142, 20, 55, 77, 20, 68, 61, 7, 219, 159, 100, 74, 241, 29,
            97, 77, 12, 175, 8, 207, 163, 196, 164, 1, 249, 170,
        ],
        programs: 1,
        program: 352950,
        decode: 5049852,
        ty: 12886,
        preparation: 6625358,
        retained: 1543665,
    },
    Calibration {
        name: "language-window8-qualified-dependency-analysis",
        digest: [
            114, 188, 35, 156, 109, 205, 244, 132, 43, 243, 61, 114, 239, 158, 254, 216, 37, 118,
            36, 213, 148, 112, 90, 135, 108, 179, 116, 97, 127, 242, 219, 20,
        ],
        programs: 1,
        program: 450181,
        decode: 8643670,
        ty: 112591,
        preparation: 11099120,
        retained: 1429120,
    },
    Calibration {
        name: "language-window8-qualified-dependency-anchor",
        digest: [
            207, 181, 219, 99, 181, 94, 214, 72, 189, 252, 67, 36, 49, 33, 224, 72, 100, 251, 219,
            240, 18, 88, 132, 77, 146, 170, 226, 151, 141, 182, 77, 130,
        ],
        programs: 1,
        program: 3011439,
        decode: 56302113,
        ty: 112591,
        preparation: 58091777,
        retained: 2104458,
    },
    Calibration {
        name: "language-window8-qualified-dependency-rebase",
        digest: [
            109, 190, 153, 10, 204, 186, 110, 215, 172, 52, 251, 80, 115, 15, 86, 141, 53, 121, 30,
            244, 169, 210, 27, 235, 133, 236, 203, 55, 29, 228, 1, 159,
        ],
        programs: 1,
        program: 166673,
        decode: 2141887,
        ty: 17103,
        preparation: 3436699,
        retained: 1339605,
    },
    Calibration {
        name: "language-window8-qualified-independent-commit",
        digest: [
            86, 69, 79, 206, 195, 25, 64, 156, 19, 161, 63, 169, 61, 73, 182, 37, 204, 192, 19,
            145, 111, 204, 148, 136, 56, 119, 64, 147, 200, 30, 51, 235,
        ],
        programs: 1,
        program: 7442494,
        decode: 140114979,
        ty: 135702,
        preparation: 152539334,
        retained: 17341680,
    },
    Calibration {
        name: "language-window8-qualified-lexical-anchor",
        digest: [
            34, 109, 132, 64, 61, 117, 76, 234, 139, 137, 107, 16, 62, 17, 240, 48, 37, 91, 116,
            138, 116, 150, 33, 77, 235, 190, 83, 91, 244, 159, 237, 23,
        ],
        programs: 1,
        program: 86237,
        decode: 1022133,
        ty: 7349,
        preparation: 3238201,
        retained: 2199467,
    },
    Calibration {
        name: "language-window8-qualified-lexical-anchor-scalars",
        digest: [
            118, 134, 144, 83, 89, 200, 50, 158, 32, 98, 142, 213, 170, 98, 204, 69, 126, 53, 38,
            55, 96, 85, 137, 108, 98, 123, 56, 103, 1, 62, 121, 87,
        ],
        programs: 1,
        program: 1810272,
        decode: 32765562,
        ty: 107190,
        preparation: 34528894,
        retained: 1953052,
    },
    Calibration {
        name: "language-window8-qualified-lexical-origin",
        digest: [
            210, 92, 73, 242, 92, 143, 127, 122, 106, 162, 17, 90, 235, 181, 174, 30, 183, 34, 179,
            60, 120, 91, 240, 240, 167, 102, 71, 174, 180, 194, 20, 71,
        ],
        programs: 1,
        program: 646355,
        decode: 11911687,
        ty: 107190,
        preparation: 13385321,
        retained: 1247023,
    },
    Calibration {
        name: "language-window8-qualified-lexical-proposal",
        digest: [
            237, 79, 118, 175, 237, 56, 7, 199, 107, 243, 169, 237, 23, 224, 137, 139, 130, 214,
            30, 132, 8, 142, 123, 0, 137, 51, 102, 194, 22, 63, 76, 204,
        ],
        programs: 1,
        program: 428583,
        decode: 7987036,
        ty: 107190,
        preparation: 10362070,
        retained: 1388772,
    },
    Calibration {
        name: "language-window8-qualified-lexical-rebase-origin",
        digest: [
            137, 207, 149, 88, 125, 67, 5, 133, 42, 73, 189, 118, 186, 99, 30, 207, 28, 140, 24,
            243, 47, 33, 169, 246, 174, 43, 168, 132, 185, 62, 42, 55,
        ],
        programs: 1,
        program: 57709,
        decode: 734921,
        ty: 24037,
        preparation: 1238129,
        retained: 422312,
    },
    Calibration {
        name: "language-window8-qualified-lexical-rebase-scalars",
        digest: [
            161, 131, 83, 53, 96, 28, 196, 233, 14, 48, 133, 155, 211, 254, 148, 234, 122, 114,
            166, 150, 46, 34, 225, 210, 171, 84, 168, 110, 89, 144, 182, 54,
        ],
        programs: 1,
        program: 151648,
        decode: 1928032,
        ty: 24037,
        preparation: 3249433,
        retained: 1321890,
    },
    Calibration {
        name: "language-window8-qualified-lexical-rebase-token",
        digest: [
            37, 178, 135, 171, 128, 186, 208, 206, 2, 163, 25, 133, 53, 79, 104, 221, 46, 184, 140,
            166, 114, 130, 194, 199, 149, 182, 218, 148, 193, 79, 57, 248,
        ],
        programs: 1,
        program: 105192,
        decode: 1344098,
        ty: 24037,
        preparation: 2520792,
        retained: 1132607,
    },
    Calibration {
        name: "language-window8-qualified-lexical-token",
        digest: [
            6, 88, 210, 225, 227, 65, 145, 140, 198, 25, 185, 82, 120, 21, 24, 200, 141, 107, 14,
            155, 150, 254, 204, 251, 231, 240, 193, 33, 174, 223, 185, 177,
        ],
        programs: 1,
        program: 657979,
        decode: 12054771,
        ty: 107190,
        preparation: 13671776,
        retained: 1382076,
    },
    Calibration {
        name: "language-window8-qualified-protected-choice",
        digest: [
            25, 150, 11, 101, 131, 160, 138, 183, 190, 133, 40, 128, 87, 8, 163, 113, 24, 252, 160,
            120, 90, 124, 106, 240, 33, 248, 35, 239, 213, 134, 198, 171,
        ],
        programs: 1,
        program: 141484,
        decode: 1907964,
        ty: 25276,
        preparation: 2023272,
        retained: 53225,
    },
    Calibration {
        name: "language-window8-rank-0-1",
        digest: [
            88, 66, 243, 144, 156, 252, 24, 84, 89, 85, 162, 140, 171, 249, 87, 94, 186, 89, 53,
            22, 76, 123, 105, 89, 110, 117, 182, 85, 35, 6, 157, 107,
        ],
        programs: 1,
        program: 1587676,
        decode: 24883091,
        ty: 44067,
        preparation: 30063855,
        retained: 4996769,
    },
    Calibration {
        name: "language-window8-rank-0-2",
        digest: [
            198, 86, 142, 228, 158, 179, 6, 212, 99, 170, 48, 50, 238, 10, 218, 7, 230, 162, 201,
            224, 254, 54, 107, 161, 124, 254, 231, 41, 186, 144, 248, 59,
        ],
        programs: 1,
        program: 1587676,
        decode: 24883091,
        ty: 44067,
        preparation: 30063855,
        retained: 4996769,
    },
    Calibration {
        name: "language-window8-rank-1-2",
        digest: [
            90, 151, 146, 124, 239, 86, 69, 128, 181, 212, 37, 21, 8, 113, 6, 112, 234, 245, 148,
            253, 177, 64, 245, 48, 15, 172, 206, 169, 76, 237, 190, 58,
        ],
        programs: 1,
        program: 1587676,
        decode: 24883091,
        ty: 44067,
        preparation: 30063855,
        retained: 4996769,
    },
    Calibration {
        name: "language-window8-rank-1-3",
        digest: [
            133, 148, 82, 97, 158, 221, 113, 9, 83, 184, 130, 15, 47, 215, 223, 89, 59, 218, 239,
            48, 52, 198, 15, 167, 58, 111, 106, 107, 18, 62, 49, 93,
        ],
        programs: 1,
        program: 1587676,
        decode: 24883091,
        ty: 44067,
        preparation: 30063855,
        retained: 4996769,
    },
    Calibration {
        name: "language-window8-rank-2-3",
        digest: [
            229, 167, 143, 247, 185, 15, 121, 20, 235, 187, 95, 243, 116, 109, 194, 26, 91, 206,
            64, 143, 229, 7, 6, 115, 205, 30, 209, 41, 51, 254, 111, 130,
        ],
        programs: 1,
        program: 1587676,
        decode: 24883091,
        ty: 44067,
        preparation: 30063855,
        retained: 4996769,
    },
    Calibration {
        name: "language-window8-rank-insert",
        digest: [
            135, 46, 40, 180, 200, 172, 166, 72, 183, 13, 212, 104, 106, 221, 74, 57, 39, 29, 142,
            112, 204, 19, 118, 28, 176, 136, 54, 150, 203, 20, 180, 89,
        ],
        programs: 1,
        program: 1544621,
        decode: 25030758,
        ty: 55171,
        preparation: 29277556,
        retained: 4036785,
    },
    Calibration {
        name: "language-window8-reanalysis",
        digest: [
            227, 148, 49, 1, 224, 217, 247, 53, 103, 168, 201, 67, 240, 214, 205, 223, 196, 219,
            179, 185, 80, 91, 209, 75, 228, 210, 28, 148, 174, 117, 232, 83,
        ],
        programs: 1,
        program: 1453707,
        decode: 24679492,
        ty: 108741,
        preparation: 27574698,
        retained: 7207485,
    },
    Calibration {
        name: "language-window8-root-count",
        digest: [
            94, 13, 135, 180, 128, 120, 170, 75, 36, 56, 85, 192, 212, 190, 81, 130, 39, 160, 196,
            18, 124, 13, 114, 8, 67, 120, 8, 100, 26, 218, 142, 220,
        ],
        programs: 1,
        program: 98813,
        decode: 1364945,
        ty: 10726,
        preparation: 1446927,
        retained: 29465,
    },
    Calibration {
        name: "language-window8-score-advance",
        digest: [
            232, 105, 174, 249, 51, 158, 149, 117, 0, 164, 209, 122, 79, 139, 167, 96, 48, 70, 110,
            75, 191, 117, 214, 146, 147, 139, 139, 237, 124, 103, 190, 20,
        ],
        programs: 1,
        program: 146545,
        decode: 2239704,
        ty: 11382,
        preparation: 3242654,
        retained: 948099,
    },
    Calibration {
        name: "language-window8-session-empty-seed",
        digest: [
            99, 236, 190, 236, 57, 166, 226, 47, 155, 31, 187, 255, 131, 172, 74, 170, 164, 44,
            194, 80, 207, 25, 180, 133, 255, 235, 231, 193, 227, 47, 82, 212,
        ],
        programs: 1,
        program: 170787,
        decode: 2389324,
        ty: 18186,
        preparation: 5693774,
        retained: 3422844,
    },
    Calibration {
        name: "language-window8-session-seed",
        digest: [
            34, 235, 123, 120, 205, 248, 214, 170, 3, 134, 119, 165, 21, 248, 245, 212, 66, 127,
            218, 24, 45, 0, 139, 74, 197, 77, 10, 157, 29, 8, 19, 208,
        ],
        programs: 1,
        program: 283709,
        decode: 4241419,
        ty: 44067,
        preparation: 8774692,
        retained: 5053468,
    },
    Calibration {
        name: "language-window8-token-codes",
        digest: [
            92, 227, 160, 115, 114, 148, 87, 119, 13, 163, 232, 30, 167, 144, 12, 45, 187, 52, 237,
            215, 108, 30, 219, 224, 43, 94, 0, 10, 56, 251, 26, 53,
        ],
        programs: 1,
        program: 1141595,
        decode: 13280351,
        ty: 2942,
        preparation: 16693319,
        retained: 3461478,
    },
    Calibration {
        name: "language-window8-wait",
        digest: [
            190, 16, 100, 237, 59, 101, 89, 166, 36, 74, 121, 233, 92, 216, 87, 164, 39, 8, 71,
            168, 244, 153, 140, 239, 15, 117, 72, 123, 101, 130, 212, 167,
        ],
        programs: 1,
        program: 74417,
        decode: 1159206,
        ty: 18643,
        preparation: 1990765,
        retained: 670886,
    },
    Calibration {
        name: "language-window8-walk-follow",
        digest: [
            28, 175, 226, 68, 7, 206, 43, 41, 97, 67, 243, 129, 129, 202, 237, 26, 131, 192, 139,
            157, 21, 94, 63, 69, 237, 248, 122, 229, 42, 108, 32, 193,
        ],
        programs: 1,
        program: 27874,
        decode: 264404,
        ty: 346,
        preparation: 342826,
        retained: 49210,
    },
    Calibration {
        name: "language-window8-walk-initialize",
        digest: [
            147, 131, 16, 234, 89, 108, 176, 1, 17, 248, 23, 224, 156, 21, 7, 170, 229, 193, 6,
            218, 253, 102, 53, 193, 130, 245, 108, 52, 80, 98, 230, 60,
        ],
        programs: 1,
        program: 1866,
        decode: 16686,
        ty: 346,
        preparation: 36573,
        retained: 10570,
    },
];
/// Runtime/history and aggregate quotas remain the enclosing owner's policy.
/// Ordinary preparation still checks every actual decoded/evaluator receipt.
pub(crate) fn limits(
    index: usize,
    mut policy: PureSourceLimits,
) -> Result<PureSourceLimits, PureSourceRefusal> {
    let c = CALIBRATION.get(index).ok_or(PureSourceRefusal::Closed)?;
    let p = PORTS.get(index).ok_or(PureSourceRefusal::Closed)?;
    let digest: [u8; 32] = Sha256::digest(p.original_programs.as_bytes()).into();
    if p.name != c.name || digest != c.digest || p.original_programs.lines().count() != c.programs {
        return Err(PureSourceRefusal::Program);
    }
    policy.maximum_program_bytes = c.program;
    policy.maximum_program_decode_bytes = c.decode;
    policy.maximum_type_encoding_bytes = c.ty;
    policy.maximum_evaluator_preparation_bytes = c.preparation;
    policy.maximum_evaluator_retained_bytes = c.retained;
    Ok(policy)
}
pub(crate) fn reservation(
    index: usize,
    family_retained: usize,
    active_native: usize,
    policy: PureSourceLimits,
) -> Result<PureSourceReservation, PureSourceRefusal> {
    let policy = limits(index, policy)?;
    PreparedParserPureSource::reservation_before_families(
        PORTS[index].original_programs,
        family_retained,
        active_native,
        policy,
    )
}

pub(crate) struct SourceProfileReservation {
    pub(crate) preparation_bytes: usize,
    pub(crate) retained_bytes: usize,
    pub(crate) history_bytes: usize,
    pub(crate) maximum_active_native_bytes: usize,
}
/// Allocates nothing. Families are owned/charged separately by the enclosing
/// factory; this aggregates new Source owners and their declared histories.
/// Summing all preparation ceilings conservatively covers retained earlier ports
/// while later ports prepare. It does not charge shared families repeatedly.
pub(crate) fn reserve_all(
    family_retained: &[usize; 54],
    active_native: &[usize; 54],
    policy: PureSourceLimits,
    maximum_preparation_bytes: usize,
    maximum_retained_bytes: usize,
    maximum_history_bytes: usize,
) -> Result<SourceProfileReservation, PureSourceRefusal> {
    let mut total = SourceProfileReservation {
        preparation_bytes: 0,
        retained_bytes: 0,
        history_bytes: 0,
        maximum_active_native_bytes: 0,
    };
    for index in 0..54 {
        let r = reservation(index, family_retained[index], active_native[index], policy)?;
        total.preparation_bytes = total
            .preparation_bytes
            .checked_add(r.source_preparation_bytes_bound)
            .ok_or(PureSourceRefusal::Pressure)?;
        total.retained_bytes = total
            .retained_bytes
            .checked_add(r.source_retained_bytes_bound)
            .ok_or(PureSourceRefusal::Pressure)?;
        total.history_bytes = total
            .history_bytes
            .checked_add(r.history_bytes_bound)
            .ok_or(PureSourceRefusal::Pressure)?;
        total.maximum_active_native_bytes = total
            .maximum_active_native_bytes
            .max(r.active_native_bytes_bound);
        if total.preparation_bytes > maximum_preparation_bytes
            || total.retained_bytes > maximum_retained_bytes
            || total.history_bytes > maximum_history_bytes
        {
            return Err(PureSourceRefusal::Pressure);
        }
    }
    Ok(total)
}
