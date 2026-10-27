//! Reading a Stardock Fences 6 snapshot (MistFences import).
//!
//! Fences 6 keeps its layout in `HKCU\Software\Stardock\Fences` and writes the same values to
//! `DailySnapshotN.xml` (in `%APPDATA%\Stardock\Fences\Backups`) as a .NET `XmlSerializer`
//! document: sections of `<ArrayOfString>` pairs. Only the top-level sections describe the
//! active monitor profile; `<PerMonitorConfig>` repeats them for other resolutions and is not
//! read.
//!
//! Group value: `id|x|y|w|h|?|Title|\\.\DISPLAYn|monX|monY|monW|monH|…|{GUID}|…`. Spaces in
//! text fields are written as backticks and the field ends in "` ". `x`/`y`/`w`/`h` are
//! virtual-screen pixels of the group's content area: the title bar sits above `y`.
//!
//! Icon value: `group|absX|absY|relX|relY|0|0|0|\\.\DISPLAYn|openCount|…`, keyed by the item's
//! path (or `::{CLSID}` for the Recycle Bin and This PC). Group -1 is a loose desktop icon.
//! `relX`/`relY` are the icon's offset inside its group, on the group's icon grid.
//!
//! The parser is pure: it never touches the file system or Fences itself.

/// One monitor of the snapshot's `ResolutionFingerprintEx`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Monitor {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
    /// Scale in percent (100 = 96 DPI).
    pub scale_pct: u32,
    /// `\\.\DISPLAY2`. Windows renumbers these between sessions, so they only link the groups
    /// of this one snapshot to its monitors.
    pub gdi_name: String,
    /// Device instance (`AOC3202#5&15dd3631&0&UID4355`) from the monitor's interface path:
    /// stable across sessions, the same id MistFences keeps (compare ignoring case).
    pub device_id: Option<String>,
}

impl Monitor {
    pub fn right(&self) -> i32 {
        self.left + self.width
    }
    pub fn bottom(&self) -> i32 {
        self.top + self.height
    }
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right() && y >= self.top && y < self.bottom()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub id: i32,
    pub title: String,
    /// Content area in virtual-screen px (the title bar is above it).
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    /// Sixth field: non-zero on a scrolled group (its scroll offset, presumably). Not imported.
    pub scroll: i32,
    /// `\\.\DISPLAYn` the group was on when the snapshot was taken.
    pub gdi_name: String,
    /// That monitor's rectangle as the group recorded it.
    pub monitor_rect: Option<[i32; 4]>,
    pub guid: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Icon {
    /// As Fences wrote it: a full path, or `::{CLSID}`.
    pub path: String,
    /// -1 for a loose icon (on the desktop, in no group).
    pub group: i32,
    pub abs_x: i32,
    pub abs_y: i32,
    pub rel_x: i32,
    pub rel_y: i32,
    pub open_count: u32,
}

impl Icon {
    pub fn is_loose(&self) -> bool {
        self.group < 0
    }
    /// Explorer's per-folder settings file, listed like an icon although it never shows.
    pub fn is_desktop_ini(&self) -> bool {
        self.path
            .rsplit(['\\', '/'])
            .next()
            .is_some_and(|n| n.eq_ignore_ascii_case("desktop.ini"))
    }
}

/// An entry that could not be read, with the reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skipped {
    pub section: &'static str,
    pub text: String,
    pub reason: &'static str,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub title: Option<String>,
    pub timestamp: Option<String>,
    pub monitors: Vec<Monitor>,
    pub groups: Vec<Group>,
    pub icons: Vec<Icon>,
    /// Top-level settings, name → value (typed values unwrapped: `<int>1</int>` → `1`).
    pub settings: Vec<(String, String)>,
    /// Rule names (`{Folders}`, …) with their target group (-1: the rule files nothing).
    pub rules: Vec<(String, i32)>,
    pub skipped: Vec<Skipped>,
}

impl Snapshot {
    pub fn setting(&self, name: &str) -> Option<&str> {
        self.settings
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn setting_int(&self, name: &str) -> Option<i64> {
        self.setting(name).and_then(|v| v.trim().parse().ok())
    }

    pub fn group(&self, id: i32) -> Option<&Group> {
        self.groups.iter().find(|g| g.id == id)
    }

    /// The monitor a group belongs to: by its GDI name, else the one holding its centre.
    pub fn monitor_of(&self, g: &Group) -> Option<&Monitor> {
        self.monitors
            .iter()
            .find(|m| m.gdi_name.eq_ignore_ascii_case(&g.gdi_name))
            .or_else(|| {
                let (cx, cy) = (g.x + g.w / 2, g.y + g.h / 2);
                self.monitors.iter().find(|m| m.contains(cx, cy))
            })
    }

    /// The monitor holding a point (a loose icon's position).
    pub fn monitor_at(&self, x: i32, y: i32) -> Option<&Monitor> {
        self.monitors.iter().find(|m| m.contains(x, y))
    }

    /// A group's icons in Fences' order: row by row, left to right.
    pub fn icons_of(&self, group: i32) -> Vec<&Icon> {
        let mut v: Vec<&Icon> = self
            .icons
            .iter()
            .filter(|i| i.group == group && !i.is_desktop_ini())
            .collect();
        v.sort_by_key(|i| (i.rel_y, i.rel_x));
        v
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    NotASnapshot,
    NoGroups,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::NotASnapshot => f.write_str("not a Fences snapshot (no <SnapshotInfo>)"),
            ParseError::NoGroups => f.write_str("the snapshot has no <Groups> section"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parses the text of a `DailySnapshotN.xml` / `PreInstallSnapshot.xml`.
pub fn parse(xml: &str) -> Result<Snapshot, ParseError> {
    let xml = xml.trim_start_matches('\u{feff}');
    if !xml.contains("<SnapshotInfo") {
        return Err(ParseError::NotASnapshot);
    }
    // The top level only: the per-monitor profiles repeat the same section names.
    let top = match xml.find("<PerMonitorConfig") {
        Some(i) => &xml[..i],
        None => xml,
    };
    let mut snap = Snapshot {
        title: element(top, "Title").map(unescape),
        timestamp: element(top, "Timestamp").map(unescape),
        ..Default::default()
    };
    if let Some(fp) = element(top, "ResolutionFingerprintEx") {
        snap.monitors = parse_fingerprint(&unescape(fp));
    }
    let groups = element(top, "Groups").ok_or(ParseError::NoGroups)?;
    for pair in string_arrays(groups) {
        match pair.as_slice() {
            // The header row: an empty key and the group count.
            [k, _] if k.is_empty() => {}
            [_, v] => match parse_group(v) {
                Some(g) => snap.groups.push(g),
                None => snap.skipped.push(Skipped {
                    section: "Groups",
                    text: v.clone(),
                    reason: "unreadable group",
                }),
            },
            other => snap.skipped.push(Skipped {
                section: "Groups",
                text: other.join(" | "),
                reason: "unexpected entry",
            }),
        }
    }
    if let Some(icons) = element(top, "Icons") {
        for pair in string_arrays(icons) {
            match pair.as_slice() {
                [path, v] if !path.is_empty() => match parse_icon(path, v) {
                    Some(i) => snap.icons.push(i),
                    None => snap.skipped.push(Skipped {
                        section: "Icons",
                        text: format!("{path} || {v}"),
                        reason: "unreadable icon position",
                    }),
                },
                other => snap.skipped.push(Skipped {
                    section: "Icons",
                    text: other.join(" | "),
                    reason: "unexpected entry",
                }),
            }
        }
    }
    if let Some(settings) = element(top, "Settings") {
        for entry in string_arrays(settings) {
            match entry.as_slice() {
                [name, value] => snap.settings.push((name.clone(), value.clone())),
                [name, _type, value] => {
                    snap.settings.push((name.clone(), typed_value(value)));
                }
                [name] => snap.settings.push((name.clone(), String::new())),
                _ => {}
            }
        }
    }
    if let Some(rules) = element(top, "Rules") {
        let entries = string_arrays(rules);
        for entry in &entries {
            let [key, value] = entry.as_slice() else {
                continue;
            };
            if key.ends_with("_Target") {
                continue;
            }
            let name = value.split('|').next().unwrap_or_default().to_string();
            let target = entries
                .iter()
                .find(|e| e.first().is_some_and(|k| *k == format!("{key}_Target")))
                .and_then(|e| e.get(1))
                .and_then(|t| t.split('|').nth(1))
                .and_then(|t| t.trim().parse().ok())
                .unwrap_or(-1);
            snap.rules.push((name, target));
        }
    }
    Ok(snap)
}

/// `[0|0|2560|1440|100|\\.\DISPLAY2|\\.\DISPLAY2\Monitor0|\\?\DISPLAY#AOC3202#5&…#{…}|…];[…];`
fn parse_fingerprint(text: &str) -> Vec<Monitor> {
    text.split(';')
        .filter_map(|m| {
            let m = m.trim().strip_prefix('[')?.strip_suffix(']')?;
            let f: Vec<&str> = m.split('|').collect();
            let num = |i: usize| f.get(i).and_then(|v| v.trim().parse::<i32>().ok());
            let device_id = f
                .iter()
                .find(|p| p.starts_with(r"\\?\DISPLAY#"))
                .and_then(|p| {
                    let mut parts = p.split('#').skip(1);
                    let model = parts.next()?;
                    let instance = parts.next()?;
                    (!model.is_empty() && !instance.is_empty())
                        .then(|| format!("{model}#{instance}"))
                });
            Some(Monitor {
                left: num(0)?,
                top: num(1)?,
                width: num(2)?,
                height: num(3)?,
                scale_pct: num(4).map_or(100, |s| s.max(1) as u32),
                gdi_name: f.get(5)?.trim().to_string(),
                device_id,
            })
        })
        .collect()
}

/// A text field: backticks stand for spaces and the field ends in "` ".
fn text_field(raw: &str) -> String {
    let raw = raw.strip_suffix(' ').unwrap_or(raw);
    let raw = raw.strip_suffix('`').unwrap_or(raw);
    raw.replace('`', " ")
}

fn parse_group(v: &str) -> Option<Group> {
    let f: Vec<&str> = v.split('|').collect();
    let num = |i: usize| f.get(i).and_then(|s| s.trim().parse::<i32>().ok());
    let title = text_field(f.get(6)?);
    let rect = (|| Some([num(8)?, num(9)?, num(10)?, num(11)?]))();
    let guid = f
        .iter()
        .map(|s| s.trim())
        .find(|s| s.starts_with('{') && s.ends_with('}') && s.len() == 38)
        .map(str::to_string);
    Some(Group {
        id: num(0)?,
        x: num(1)?,
        y: num(2)?,
        w: num(3)?,
        h: num(4)?,
        scroll: num(5).unwrap_or(0),
        title,
        gdi_name: text_field(f.get(7).copied().unwrap_or_default()),
        monitor_rect: rect,
        guid,
    })
}

fn parse_icon(path: &str, v: &str) -> Option<Icon> {
    let f: Vec<&str> = v.split('|').collect();
    let num = |i: usize| f.get(i).and_then(|s| s.trim().parse::<i32>().ok());
    Some(Icon {
        path: path.to_string(),
        group: num(0)?,
        abs_x: num(1)?,
        abs_y: num(2)?,
        rel_x: num(3)?,
        rel_y: num(4)?,
        open_count: num(9).map_or(0, |n| n.max(0) as u32),
    })
}

/// `<?xml …?><int>1</int>` (already unescaped) → `1`.
fn typed_value(v: &str) -> String {
    let v = v.trim_start_matches('\u{feff}');
    let body = match v.find("?>") {
        Some(i) => &v[i + 2..],
        None => v,
    };
    let inner = body
        .find('>')
        .and_then(|a| body.rfind("</").map(|b| (a, b)))
        .filter(|(a, b)| a < b)
        .map(|(a, b)| &body[a + 1..b]);
    inner.unwrap_or(body).trim().to_string()
}

/// The text between the first `<name>` and its `</name>`; `None` when absent.
fn element<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(&xml[start..end])
}

/// Every `<ArrayOfString>` in `xml` as its unescaped `<string>` values (`<string />` = "").
fn string_arrays(xml: &str) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<ArrayOfString>") {
        let body_start = start + "<ArrayOfString>".len();
        let Some(len) = rest[body_start..].find("</ArrayOfString>") else {
            break;
        };
        let body = &rest[body_start..body_start + len];
        out.push(strings(body));
        rest = &rest[body_start + len + "</ArrayOfString>".len()..];
    }
    out
}

fn strings(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = body;
    while let Some(i) = rest.find("<string") {
        rest = &rest[i + "<string".len()..];
        let Some(gt) = rest.find('>') else { break };
        if rest[..gt].trim_end().ends_with('/') {
            out.push(String::new());
            rest = &rest[gt + 1..];
            continue;
        }
        let content = &rest[gt + 1..];
        let Some(end) = content.find("</string>") else {
            break;
        };
        out.push(unescape(&content[..end]));
        rest = &content[end + "</string>".len()..];
    }
    out
}

fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(semi) = rest.find(';').filter(|&j| j <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..semi];
        let ch = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            e if e.starts_with("#x") || e.starts_with("#X") => u32::from_str_radix(&e[2..], 16)
                .ok()
                .and_then(char::from_u32),
            e if e.starts_with('#') => e[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A trimmed copy of a real snapshot's shape (names and paths made up).
    const SAMPLE: &str = "\u{feff}<?xml version=\"1.0\" encoding=\"utf-8\"?>
<SnapshotInfo xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\">
  <GUID>DailySnapshot6</GUID>
  <Title>Daily Backup (21-10-2026)</Title>
  <Timestamp>2026-10-21T16:01:17.025372+03:00</Timestamp>
  <ResolutionFingerprintEx>[0|0|2560|1440|100|\\\\.\\DISPLAY2|\\\\.\\DISPLAY2\\Monitor0|\\\\?\\DISPLAY#AOC3202#5&amp;15dd3631&amp;0&amp;UID4355#{e6f07b5f-ee97-4a90-b076-33f57bf4eaa7}|\\Registry\\Machine\\0002];[2560|0|1920|1080|100|\\\\.\\DISPLAY1|\\\\.\\DISPLAY1\\Monitor0|\\\\?\\DISPLAY#AOC2702#5&amp;15dd3631&amp;0&amp;UID4353#{e6f07b5f-ee97-4a90-b076-33f57bf4eaa7}|\\Registry\\Machine\\0001];</ResolutionFingerprintEx>
  <Groups>
    <ArrayOfString>
      <string />
      <string>3</string>
    </ArrayOfString>
    <ArrayOfString>
      <string>0</string>
      <string>3|889|36|910|457|0|Игры` |\\\\.\\DISPLAY2` |0|0|2560|1440|0|0|255|0|` |-1|0|1|0|0|{BEA23395-174E-40E5-AC76-DCAFE96CD1F7} |0|0|0|</string>
    </ArrayOfString>
    <ArrayOfString>
      <string>1</string>
      <string>5|330|36|548|457|182|Лаунчеры`и`тд` |\\\\.\\DISPLAY2` |0|0|2560|1440|0|0|255|0|` |-1|0|1|0|0|{B851BA83-AC5C-445A-AFA6-ABCF603267C7} |0|0|0|</string>
    </ArrayOfString>
    <ArrayOfString>
      <string>2</string>
      <string>8|2591|43|910|1640|0|Учёба` |\\\\.\\DISPLAY1` |2560|0|1920|1080|0|0|255|0|` |-1|0|1|0|0|{3716DA39-5E8A-47E6-9827-C923301EDA6E} |0|0|0|</string>
    </ArrayOfString>
    <ArrayOfString>
      <string>3</string>
      <string>broken</string>
    </ArrayOfString>
  </Groups>
  <Icons>
    <ArrayOfString>
      <string>C:\\Users\\A\\Desktop\\B.url</string>
      <string>3|1001|38|112|2|0|0|0|\\\\.\\DISPLAY2|284|1|2||0|3|4</string>
    </ArrayOfString>
    <ArrayOfString>
      <string>C:\\Users\\A\\Desktop\\A.url</string>
      <string>3|910|38|21|2|0|0|0|\\\\.\\DISPLAY2|0|1|2|Explorer.EXE|0|0|0</string>
    </ArrayOfString>
    <ArrayOfString>
      <string>C:\\Users\\A\\Desktop\\C &amp; D.lnk</string>
      <string>3|910|129|21|93|0|0|0|\\\\.\\DISPLAY2|7|1|2|Explorer.EXE|0|0|0</string>
    </ArrayOfString>
    <ArrayOfString>
      <string>::{645FF040-5081-101B-9F08-00AA002F954E}</string>
      <string>-1|1588|497|1588|497|0|0|0|\\\\.\\DISPLAY2|58|0|0||0|2147483647|31281620</string>
    </ArrayOfString>
    <ArrayOfString>
      <string>C:\\Users\\A\\Desktop\\desktop.ini</string>
      <string>-1|0|156|0|156|0|0|0|\\\\.\\DISPLAY2`|0|0|0|Explorer.EXE|0|0|0|</string>
    </ArrayOfString>
  </Icons>
  <Settings>
    <ArrayOfString>
      <string>RollupOnDock</string>
      <string>System.Int32</string>
      <string>\u{feff}&lt;?xml version=\"1.0\" encoding=\"utf-8\"?&gt;&lt;int&gt;1&lt;/int&gt;</string>
    </ArrayOfString>
    <ArrayOfString>
      <string>DesktopPath</string>
      <string>C:\\Users\\A\\Desktop</string>
    </ArrayOfString>
  </Settings>
  <Rules>
    <ArrayOfString>
      <string>00</string>
      <string>{Folders}|&lt;type&gt;:D:0:|</string>
    </ArrayOfString>
    <ArrayOfString>
      <string>00_Target</string>
      <string>0|-1|1|10</string>
    </ArrayOfString>
  </Rules>
  <PerMonitorConfig>
    <SnapshotInfoPerMonitorConfig>
      <Groups>
        <ArrayOfString>
          <string>9</string>
          <string>9|1|1|100|100|0|Other`profile` |\\\\.\\DISPLAY2` |0|0|2048|1536|0|</string>
        </ArrayOfString>
      </Groups>
    </SnapshotInfoPerMonitorConfig>
  </PerMonitorConfig>
</SnapshotInfo>";

    #[test]
    fn reads_monitors_groups_icons_settings_and_rules() {
        let s = parse(SAMPLE).unwrap();
        assert_eq!(
            s.timestamp.as_deref(),
            Some("2026-10-21T16:01:17.025372+03:00")
        );
        assert_eq!(s.monitors.len(), 2);
        assert_eq!(s.monitors[0].gdi_name, r"\\.\DISPLAY2");
        assert_eq!(
            s.monitors[0].device_id.as_deref(),
            Some("AOC3202#5&15dd3631&0&UID4355")
        );
        assert_eq!((s.monitors[1].left, s.monitors[1].width), (2560, 1920));
        // The per-monitor profile is not read; the broken group is reported, not dropped.
        let titles: Vec<&str> = s.groups.iter().map(|g| g.title.as_str()).collect();
        assert_eq!(titles, ["Игры", "Лаунчеры и тд", "Учёба"]);
        assert_eq!(s.skipped.len(), 1);
        assert_eq!(s.skipped[0].reason, "unreadable group");
        let g = s.group(8).unwrap();
        assert_eq!((g.x, g.y, g.w, g.h), (2591, 43, 910, 1640));
        assert_eq!(g.gdi_name, r"\\.\DISPLAY1");
        assert_eq!(g.monitor_rect, Some([2560, 0, 1920, 1080]));
        assert_eq!(
            s.monitor_of(g).unwrap().device_id.as_deref(),
            Some("AOC2702#5&15dd3631&0&UID4353")
        );
        assert_eq!(s.group(5).unwrap().scroll, 182);
        assert_eq!(s.icons.len(), 5);
        assert!(
            s.icons
                .iter()
                .any(|i| i.path == r"C:\Users\A\Desktop\C & D.lnk")
        );
        assert_eq!(s.setting_int("RollupOnDock"), Some(1));
        assert_eq!(s.setting("DesktopPath"), Some(r"C:\Users\A\Desktop"));
        assert_eq!(s.rules, vec![("{Folders}".to_string(), -1)]);
    }

    #[test]
    fn group_icons_come_in_rows_and_skip_desktop_ini() {
        let s = parse(SAMPLE).unwrap();
        let order: Vec<&str> = s
            .icons_of(3)
            .iter()
            .map(|i| i.path.rsplit('\\').next().unwrap())
            .collect();
        assert_eq!(order, ["A.url", "B.url", "C & D.lnk"]);
        assert_eq!(s.icons_of(3)[1].open_count, 284);
        let loose = s.icons_of(-1);
        assert_eq!(loose.len(), 1);
        assert!(loose[0].path.starts_with("::{645FF040"));
    }

    #[test]
    fn rejects_other_files() {
        assert_eq!(parse("<Config/>"), Err(ParseError::NotASnapshot));
        assert_eq!(
            parse("<SnapshotInfo></SnapshotInfo>"),
            Err(ParseError::NoGroups)
        );
    }

    #[test]
    fn unescapes_entities() {
        assert_eq!(
            unescape("a &amp; b &lt;c&gt; &#1067; &#x44B;"),
            "a & b <c> Ы ы"
        );
        assert_eq!(unescape("AT&T"), "AT&T");
        assert_eq!(text_field("Видео`и`стримы` "), "Видео и стримы");
        assert_eq!(text_field(r"\\.\DISPLAY2` "), r"\\.\DISPLAY2");
    }
}
