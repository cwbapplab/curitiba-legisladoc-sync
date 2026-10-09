//! Runtime schema, read from the feed's own `*_Legisladoc_-_Info.xml`.
//!
//! The published CSV layout is not documented anywhere else, and the confirmed
//! live columns match the XML's non-deleted `<Column index=...>` entries
//! one-for-one and in order. Reading the schema instead of hard-coding it means
//! a snapshot that adds a column is picked up rather than silently truncated.

use anyhow::{Context, Result};

/// One column of a table, as declared in `Info.xml`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Column {
    /// 1-based position in the CSV, from the `index` attribute.
    pub index: u32,
    pub name: String,
    /// Declared SQL type: `int`, `varchar`, `datetime`, `bit`, ...
    pub sql_type: String,
    pub size: Option<u32>,
    pub nullable: bool,
    /// Human description from the `help` attribute — used as the UI field label.
    pub help: Option<String>,
    /// Enum/flag values the column is restricted to.
    pub values: Vec<EnumValue>,
    /// Foreign key, e.g. `TIPO_ATO.ID`.
    pub related: Option<Related>,
    /// Header name used by older exports (the xlsx dictionary uses these).
    pub old_name: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EnumValue {
    pub value: String,
    pub description: String,
    pub help: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Related {
    pub table: String,
    pub key: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Table {
    pub name: String,
    /// Live columns only, ordered by `index`.
    pub columns: Vec<Column>,
    /// Columns the feed dropped (`operation="deleted"`); kept for diagnostics.
    pub deleted_columns: Vec<String>,
}

impl Table {
    /// The column holding the primary key, if the schema declares one.
    ///
    /// `Info.xml` does not mark keys, so this uses the convention every table in
    /// this feed follows: a column literally named `ID`.
    pub fn primary_key(&self) -> Option<&Column> {
        self.columns.iter().find(|c| c.name == "ID")
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Schema {
    pub version: Option<String>,
    pub provider: Option<String>,
    pub tables: Vec<Table>,
}

impl Schema {
    pub fn table(&self, name: &str) -> Option<&Table> {
        self.tables.iter().find(|t| t.name == name)
    }

    /// Parse `Info.xml`.
    ///
    /// The file declares itself as `utf-8` and the ASCII/XML parts are clean;
    /// the Portuguese accents in `help` attributes are the only non-ASCII, and
    /// roxmltree decodes those via the declared encoding.
    pub fn parse(xml: &[u8]) -> Result<Self> {
        // The declaration says utf-8 but the bytes are Windows-1252 in places;
        // decoding first avoids roxmltree rejecting the document.
        let text = decode(xml);
        let doc = roxmltree::Document::parse(&text)
            .context("Info.xml is not valid XML")?;
        let root = doc.root_element();

        let mut tables = Vec::new();
        for t in root.descendants().filter(|n| n.has_tag_name("Table")) {
            let name = t.attribute("name").unwrap_or_default().to_string();
            let mut columns = Vec::new();
            let mut deleted_columns = Vec::new();

            // Columns sit inside a `<Columns>` wrapper, so this walks descendants
            // rather than direct children.
            for c in t.descendants().filter(|n| n.has_tag_name("Column")) {
                let cname = c.attribute("name").unwrap_or_default().to_string();
                match c.attribute("operation") {
                    // `deleted` columns are listed for changelog purposes only
                    // and are absent from the published CSV.
                    Some("deleted") => {
                        deleted_columns.push(cname);
                        continue;
                    }
                    _ => {}
                }

                let values = c
                    .children()
                    .find(|n| n.has_tag_name("Values"))
                    .map(|vs| {
                        vs.children()
                            .filter(|n| n.has_tag_name("Value"))
                            .map(|v| {
                                // `<Value description="false" help="...">0</Value>`
                                // — the element text is the stored value.
                                let value = v
                                    .text()
                                    .map(str::trim)
                                    .unwrap_or_default()
                                    .to_string();
                                EnumValue {
                                    value,
                                    description: v
                                        .attribute("description")
                                        .unwrap_or_default()
                                        .to_string(),
                                    help: v.attribute("help").map(str::to_string),
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let related = c.children().find(|n| n.has_tag_name("Related")).map(|r| {
                    Related {
                        table: r.attribute("table").unwrap_or_default().to_string(),
                        key: r.attribute("key").unwrap_or_default().to_string(),
                    }
                });

                columns.push(Column {
                    index: c
                        .attribute("index")
                        .and_then(|i| i.parse().ok())
                        .unwrap_or(0),
                    name: cname,
                    sql_type: c.attribute("type").unwrap_or("varchar").to_string(),
                    size: c.attribute("size").and_then(|s| s.parse().ok()),
                    // Absent `nullable` means the source did not say; assume true.
                    nullable: c.attribute("nullable") != Some("false"),
                    help: c.attribute("help").map(str::to_string),
                    values,
                    related,
                    old_name: c.attribute("old-name").map(str::to_string),
                });
            }

            // `index` is the CSV position, so sorting by it gives the column order.
            columns.sort_by_key(|c| c.index);
            tables.push(Table {
                name,
                columns,
                deleted_columns,
            });
        }

        let tables_el = root.descendants().find(|n| n.has_tag_name("Tables"));
        Ok(Schema {
            version: root.attribute("version").map(str::to_string),
            provider: tables_el.and_then(|t| t.attribute("provider")).map(str::to_string),
            tables,
        })
    }
}

/// Decode as UTF-8, falling back to Windows-1252 (the feed's real encoding for
/// accented text, e.g. `Publicação`).
fn decode(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => {
            let (cow, _, _) = encoding_rs::WINDOWS_1252.decode(bytes);
            cow.into_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="utf-8" ?>
<Info version="2.0.0">
  <Tables provider="MSSQL">
    <Table operation="changed" name="ATO" source="dbo.VW_ATO">
      <Columns>
        <Column operation="changed" index="02" name="NUMERO" type="varchar" size="20" nullable="false" help="Número do Ato"></Column>
        <Column operation="changed" index="01" name="ID" type="int" nullable="false" help="Chave"></Column>
        <Column operation="deleted" name="DROPPED" type="varchar"></Column>
        <Column operation="changed" index="03" name="TRAMITAVEL" type="bit" nullable="false">
          <Values type="enum"><Value description="false" help="Não tramitável">0</Value><Value description="true">1</Value></Values>
        </Column>
        <Column operation="created" index="04" name="TIPO_ID" type="smallint" nullable="false">
          <Related table="TIPO_ATO" key="ID" />
        </Column>
      </Columns>
    </Table>
    <Table operation="created" name="ORGAO">
      <Columns><Column operation="created" index="01" name="ID" type="smallint" nullable="false"></Column></Columns>
    </Table>
  </Tables>
</Info>"#;

    #[test]
    fn parses_tables_columns_and_orders_by_index() {
        let s = Schema::parse(SAMPLE.as_bytes()).unwrap();
        assert_eq!(s.version.as_deref(), Some("2.0.0"));
        assert_eq!(s.provider.as_deref(), Some("MSSQL"));
        assert_eq!(s.tables.len(), 2);

        let ato = s.table("ATO").unwrap();
        // `index` decides order, not document order.
        let names: Vec<_> = ato.columns.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["ID", "NUMERO", "TRAMITAVEL", "TIPO_ID"]);
        // Deleted columns are recorded but excluded from the live set.
        assert_eq!(ato.deleted_columns, ["DROPPED"]);
        assert_eq!(ato.primary_key().unwrap().name, "ID");
    }

    #[test]
    fn captures_types_help_enums_and_relations() {
        let s = Schema::parse(SAMPLE.as_bytes()).unwrap();
        let ato = s.table("ATO").unwrap();

        let numero = &ato.columns[1];
        assert_eq!(numero.sql_type, "varchar");
        assert_eq!(numero.size, Some(20));
        assert!(!numero.nullable);
        assert_eq!(numero.help.as_deref(), Some("Número do Ato"));

        let tram = &ato.columns[2];
        assert_eq!(tram.values.len(), 2);
        assert_eq!(tram.values[0].value, "0");
        assert_eq!(tram.values[0].description, "false");
        assert_eq!(tram.values[0].help.as_deref(), Some("Não tramitável"));

        let tipo = &ato.columns[3];
        let rel = tipo.related.as_ref().unwrap();
        assert_eq!(rel.table, "TIPO_ATO");
        assert_eq!(rel.key, "ID");
    }

    #[test]
    fn missing_nullable_attribute_is_treated_as_nullable() {
        let xml = r#"<Info><Tables><Table name="T"><Columns>
            <Column index="01" name="A" type="varchar"></Column>
        </Columns></Table></Tables></Info>"#;
        let s = Schema::parse(xml.as_bytes()).unwrap();
        assert!(s.table("T").unwrap().columns[0].nullable);
    }

    #[test]
    fn a_broken_document_is_an_error_not_a_panic() {
        assert!(Schema::parse(b"<Info><Tables>").is_err());
    }
}
