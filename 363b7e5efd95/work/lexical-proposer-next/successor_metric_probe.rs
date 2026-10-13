use serde_json::{json,Value};
fn metrics(rows: &[Value], receipts: &[Value]) -> Value {
    let mut tokens = 0usize;
    let mut heads = 0;
    let mut labels = 0;
    let mut pos = 0;
    let mut exact = 0;
    let mut refusals = 0;
    let (mut tp, mut fp, mut fn_) = (0, 0, 0);
    for (row, receipt) in rows.iter().zip(receipts) {
        let count = row["forms"].as_array().unwrap().len();
        tokens += count;
        let refused = receipt.get("refusal").is_some();
        refusals += usize::from(refused);
        let mut matched = !refused && receipt["complete"] == true;
        for i in 0..count {
            let gold = row["relations"][i]
                .as_str()
                .unwrap()
                .split(':')
                .next()
                .unwrap();
            let predicted = if refused {
                None
            } else {
                receipt["predicted"][i]["base"].as_str()
            };
            let head = !refused && receipt["predicted"][i]["head"] == row["heads"][i];
            let relation = head && predicted == Some(gold);
            let tag = !refused && receipt["pos"][i] == row["pos"][i];
            heads += usize::from(head);
            labels += usize::from(relation);
            pos += usize::from(tag);
            matched &= head && relation && tag;
            let p = predicted == Some("vocative");
            let g = gold == "vocative";
            let hit = p && g && head;
            tp += usize::from(hit);
            fp += usize::from(p && !hit);
            fn_ += usize::from(g && !hit);
        }
        exact += usize::from(matched);
    }
    json!({"tokens":tokens,"correct_heads":heads,"correct_base_labels":labels,"correct_pos":pos,"exact_graphs":exact,"refused_rows":refusals,"uas":heads as f64/tokens as f64,"base_las":labels as f64/tokens as f64,"pos_accuracy":pos as f64/tokens as f64,"vocative_tp":tp,"vocative_fp":fp,"vocative_fn":fn_,"vocative_precision":if tp+fp==0 {None}else{Some(tp as f64/(tp+fp) as f64)},"vocative_recall":if tp+fn_==0 {None}else{Some(tp as f64/(tp+fn_) as f64)},"token_scope":"all supplied original tokens, including refused rows and punctuation"})
}
#[test]
fn refused_rows_keep_original_token_and_vocative_denominators() {
    let rows = vec![
        json!({"forms":["I"],"heads":[8],"relations":["root"],"pos":[7]}),
        json!({"forms":["Morgan"],"heads":[0],"relations":["vocative"],"pos":[11]}),
    ];
    let receipts = vec![
        json!({"complete":true,"predicted":[{"head":8,"base":"root"}],"pos":[7]}),
        json!({"refusal":"original tokenization differs"}),
    ];
    let result = metrics(&rows, &receipts);
    assert_eq!(result["tokens"], 2);
    assert_eq!(result["uas"], 0.5);
    assert_eq!(result["base_las"], 0.5);
    assert_eq!(result["pos_accuracy"], 0.5);
    assert_eq!(result["vocative_fn"], 1);
    assert_eq!(result["refused_rows"], 1);
    assert_eq!(result["exact_graphs"], 1);
}
