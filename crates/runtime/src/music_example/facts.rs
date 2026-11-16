use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FactBlock {
    pub ref_id: String,
    pub markdown: String,
    pub source_ref_ids: Vec<String>,
    pub data: Value,
}
impl FactBlock {
    pub fn marker(&self) -> String { format!("[[musicfacts:{}]]", self.ref_id) }
}
fn text(value: &Value) -> String {
    if let Some(number)=value.as_f64().filter(|n| n.fract()==0.0) { return format!("{number:.0}"); }
    value.as_str().map(str::to_owned).unwrap_or_else(|| value.to_string())
}
fn name(note: &Value) -> String {
    if note["rest"] == true { return "休止".into(); }
    note["written"].as_str().map(str::to_owned).unwrap_or_else(|| note["sounding_midi"].as_u64().map(|m| format!("实音 {}", pitch_name(m as u8))).unwrap_or_else(|| "音高未标定".into()))
}
fn interval_label(value: &Value) -> Option<String> {
    let number=value["number"].as_u64()?;
    let quality=match value["quality"].as_str()? { "major"=>"大", "minor"=>"小", "perfect"=>"纯", "augmented"=>"增", "diminished"=>"减", _=>return None };
    let direction=match value["direction"].as_str()? { "ascending"=>"上行", "descending"=>"下行", _=>"同音" };
    let number= ["","一","二","三","四","五","六","七","八","九","十","十一","十二","十三","十四","十五"].get(number as usize).map(|s|s.to_string()).unwrap_or_else(||number.to_string());
    Some(format!("{direction}{quality}{number}度（{} 半音）",text(&value["semitones"])))
}
pub(crate) fn score_facts(all: &[MusicScoreObject], ids: &[String], source: &str, title: &str) -> FactBlock {
    let selected: Vec<_> = all.iter().filter(|n| ids.contains(&n.source_id)).collect();
    let data=read_tools::music::facts::passage(all,&selected);
    let title=title.strip_prefix("谱面合成 · ").unwrap_or(title);
    let mut markdown=format!("\n\n**{} · 谱面事实**\n\n| 小节 | 记谱音与时值（同起点音用 + 连接） | 实音 |\n| --- | --- | --- |\n", title.replace('|',"／").replace(['\r','\n']," "));
    for measure in data["measures"].as_array().into_iter().flatten() {
        let cells=measure["onsets"].as_array().into_iter().flatten().map(|g| {
            let notes=g["notes"].as_array().unwrap().iter().map(|n| {
                let tied=if matches!(n["tie"].as_str(),Some("stop"|"continue")) { "，延续前音" } else { "" };
                let tremolo=if n["rhythm"]["tremolos"].as_array().is_some_and(|v|!v.is_empty()) { "，震音" } else { "" };
                format!("{}（{}{}{}）",name(n),text(&n["value"]),tied,tremolo)
            }).collect::<Vec<_>>().join(" + ");
            format!("{}：{notes}",text(&g["beat"]))
        }).collect::<Vec<_>>().join(" → ");
        let sounding=measure["onsets"].as_array().into_iter().flatten().map(|g|g["notes"].as_array().unwrap().iter().map(|n|n["sounding"].as_str().unwrap_or("休止")).collect::<Vec<_>>().join(" + ")).collect::<Vec<_>>().join(" → ");
        markdown.push_str(&format!("| {}{} | {cells} | {sounding} |\n",text(&measure["measure"]),if measure["complete"]==true { "" } else { "（片段）" }));
    }
    markdown.push_str("\n起点以四分音符为 1，从小节内 0 起算；延音线延续音不重新起音。\n");
    if let Some(label)=interval_label(&data["endpoint_span"]["written_interval"]) {
        markdown.push_str(&format!("\n选段首末音 {} → {}：{label}。\n",text(&data["endpoint_span"]["from"]),text(&data["endpoint_span"]["to"])));
    }
    for tie in data["complete_ties"].as_array().into_iter().flatten() {
        markdown.push_str(&format!("\n从选段起点后 {} 个四分音符单位起的完整延音线链，总时值为 {} 个四分音符单位。\n",text(&tie["onset_beats"]),text(&tie["duration_beats"])));
    }
    FactBlock { ref_id:format!("facts_{}",uuid::Uuid::now_v7().simple()),markdown,source_ref_ids:vec![source.into()],data }
}
pub(crate) fn comparison_facts(left: &FactBlock, right: &FactBlock) -> FactBlock {
    let data=read_tools::music::facts::compare(&left.data,&right.data);
    let mut markdown=format!("{}\n{}\n**两段谱面比较**\n\n",left.markdown,right.markdown);
    if data["rhythm_equal"]==true {
        markdown.push_str("选定两段的起点间距、记谱时值、休止与重新起音位置相同。");
        if data["tie_pattern_equal"]!=true { markdown.push_str("延音线标记不全相同，边界外的持续方式需另查后续谱面。"); }
        markdown.push_str("\n\n| 第一段记谱音 | 第二段记谱音 | 实音半音差（第二段 − 第一段） | 音级位移后的八度差 |\n| --- | --- | --- | --- |\n");
        for pair in data["correspondence"].as_array().into_iter().flatten() {
            markdown.push_str(&format!("| {} | {} | {} | {} |\n",text(&pair["from"]),text(&pair["to"]),text(&pair["semitones"]),text(&pair["register_octaves"])));
        }
        if !data["constant_pitch_class_shift"].is_null() {
            markdown.push_str(&format!("\n所有对应音的音级位移相同：{} 半音（按八度等价，取 −6 至 5）。",text(&data["constant_pitch_class_shift"])));
            if data["constant_semitones"].is_null() { markdown.push_str("实音的半音差并不恒定，音区发生了不同的八度调整。"); }
            else { markdown.push_str(&format!("实音位移也恒定：{} 半音。",text(&data["constant_semitones"]))); }
        }
    } else { markdown.push_str("选定两段的节奏／起点结构不同，未自动逐音配对；对应关系需要缩小到明确的子片段后再比较。"); }
    markdown.push_str("\n\n以上比较限于选定谱面，震音笔画未展开；是否构成主题衍生需要结合上下文解释。\n\n");
    let mut refs=left.source_ref_ids.clone(); for id in &right.source_ref_ids { if !refs.contains(id) { refs.push(id.clone()); } }
    FactBlock { ref_id:format!("facts_{}",uuid::Uuid::now_v7().simple()),markdown,source_ref_ids:refs,data }
}
