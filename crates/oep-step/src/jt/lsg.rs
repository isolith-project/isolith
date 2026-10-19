//! LSG segment element stream traversal (implementing detail).
//!
//! The LSG block (kind 1) is walked per Siemens JT v10 Ref 6 and JTIAP:
//! graph elements that use the compressed element header form (Figure 19)
//! until the end-of-elements marker GUID, then the property table (Ref 6.3).
//! This module is private; it returns only vendor-neutral facts.

use super::core::BlockEntry;
use super::{BoundingBox, Cursor, Error, FacetMesh, PartNode};
use std::collections::HashMap;

/// End-of-elements marker GUID (Annex A Table A.1).
const END_OF_ELEMENTS: [u8; 16] = [0xFF; 16];

/// Object type GUIDs this wedge decodes (Table A.1).
const GID_PARTITION: [u8; 16] = guid_const(
    0x10DD103E,
    0x2AC8,
    0x11D1,
    [0x9B, 0x6B, 0x00, 0x80, 0xC7, 0xBB, 0x59, 0x97],
);
const GID_PART: [u8; 16] = guid_const(
    0xCE357244,
    0x38FB,
    0x11D1,
    [0xA5, 0x06, 0x00, 0x60, 0x97, 0xBD, 0xC6, 0xE1],
);
const GID_GROUP: [u8; 16] = guid_const(
    0x10DD101B,
    0x2AC8,
    0x11D1,
    [0x9B, 0x6B, 0x00, 0x80, 0xC7, 0xBB, 0x59, 0x97],
);
const GID_INSTANCE: [u8; 16] = guid_const(
    0x10DD102A,
    0x2AC8,
    0x11D1,
    [0x9B, 0x6B, 0x00, 0x80, 0xC7, 0xBB, 0x59, 0x97],
);
const GID_LOD: [u8; 16] = guid_const(
    0x10DD102C,
    0x2AC8,
    0x11D1,
    [0x9B, 0x6B, 0x00, 0x80, 0xC7, 0xBB, 0x59, 0x97],
);
const GID_SWITCH: [u8; 16] = guid_const(
    0x10DD10F3,
    0x2AC8,
    0x11D1,
    [0x9B, 0x6B, 0x00, 0x80, 0xC7, 0xBB, 0x59, 0x97],
);
const GID_STRING_PROP: [u8; 16] = guid_const(
    0x10DD106E,
    0x2AC8,
    0x11D1,
    [0x9B, 0x6B, 0x00, 0x80, 0xC7, 0xBB, 0x59, 0x97],
);
const GID_LATE_LOAD: [u8; 16] = guid_const(
    0xE0B05BE5,
    0xFBBD,
    0x11D1,
    [0xA3, 0xA7, 0x00, 0xAA, 0x00, 0xD1, 0x09, 0x54],
);

const BASE_PROPERTY: u8 = 5;

/// Compile-time GUID constant (U32 + 2xU16 + 8xU8, LSB-first words).
const fn guid_const(u32: u32, a: u16, b: u16, tail: [u8; 8]) -> [u8; 16] {
    let mut g = [0u8; 16];
    g[0] = u32 as u8;
    g[1] = (u32 >> 8) as u8;
    g[2] = (u32 >> 16) as u8;
    g[3] = (u32 >> 24) as u8;
    g[4] = a as u8;
    g[5] = (a >> 8) as u8;
    g[6] = b as u8;
    g[7] = (b >> 8) as u8;
    let mut i = 0;
    while i < 8 {
        g[8 + i] = tail[i];
        i += 1;
    }
    g
}

fn guid_eq(a: &[u8; 16], b: &[u8; 16]) -> bool {
    a == b
}

/// One decoded element: type id + base type + object id + raw object data.
pub(crate) struct Element {
    type_id: [u8; 16],
    base_type: u8,
    object_id: i32,
    data: Vec<u8>,
}

/// Read the next element in a *compressed-form* element stream (LSG kind).
/// The first element is preceded by the U32 compression flag (Figure 19).
/// Returns `None` once the end-of-elements marker is consumed.
fn read_compressed_element(c: &mut Cursor, first: bool) -> Result<Option<Element>, Error> {
    let rem = c.remaining();
    if rem == 0 {
        return Ok(None);
    }
    if rem >= 16 && c.buf[c.pos..c.pos + 16] == END_OF_ELEMENTS {
        c.pos += 16;
        return Ok(None);
    }
    if first {
        let _flag = c.u32()?; // compression flag (0 = none); tolerant
    }
    let _compressed_len = c.i32()?;
    let algorithm = c.u8()?;
    if algorithm != 1 {
        return Err(Error::UnsupportedFeature(format!(
            "compression algorithm {} (documented 1=none, 3=LZMA); LZMA payloads are a later milestone",
            algorithm
        )));
    }
    let element_len = c.i32()?;
    if element_len < 21 {
        return Err(Error::BadStructure(format!(
            "element length {element_len} too small for a header"
        )));
    }
    let type_id = super::core::read_guid(c)?;
    let base_type = c.u8()?;
    let object_id = c.i32()?;
    let data_len = (element_len - 21) as usize; // 21 = guid(16)+base(1)+id(4)
    let start = c.pos;
    let data = c
        .buf
        .get(start..start + data_len)
        .ok_or(Error::Truncated)?
        .to_vec();
    c.pos += data_len;
    Ok(Some(Element {
        type_id,
        base_type,
        object_id,
        data,
    }))
}

/// Read a plain-form element (used in shape LOD blocks, which do not support
/// whole-segment compression, Ref Table 6).
pub(crate) fn read_plain_element(c: &mut Cursor) -> Result<Option<Element>, Error> {
    let rem = c.remaining();
    if rem == 0 {
        return Ok(None);
    }
    if rem >= 16 && c.buf[c.pos..c.pos + 16] == END_OF_ELEMENTS {
        c.pos += 16;
        return Ok(None);
    }
    let element_len = c.i32()?;
    if element_len < 21 {
        return Err(Error::BadStructure(format!(
            "shape element length {element_len} too small"
        )));
    }
    let type_id = super::core::read_guid(c)?;
    let base_type = c.u8()?;
    let object_id = c.i32()?;
    let data_len = (element_len - 21) as usize;
    let start = c.pos;
    let data = c
        .buf
        .get(start..start + data_len)
        .ok_or(Error::Truncated)?
        .to_vec();
    c.pos += data_len;
    Ok(Some(Element {
        type_id,
        base_type,
        object_id,
        data,
    }))
}

/// Object tables recovered from the LSG: object-id-indexed nodes and atoms.
pub(crate) struct LsgFacts {
    /// child ids + declared transformed bbox per part node, by object id
    pub parts: HashMap<i32, (Vec<i32>, Option<BoundingBox>)>,
    /// string property value by object id
    pub names: HashMap<i32, String>,
    /// late-loaded reference (block guid, block kind, payload id) by object id
    pub late_load: HashMap<i32, ([u8; 16], u32, i32)>,
    /// property table: element object id -> value atom ids
    pub props_of: HashMap<i32, Vec<i32>>,
    pub root_children: Vec<i32>,
    pub warnings: Vec<String>,
}

fn group_children(data: &[u8], big: bool) -> Result<Vec<i32>, Error> {
    let mut c = Cursor::new(data, big);
    // Base Node Data: U8 version + U32 flags + I32 attr count + attr ids
    let _ver = c.u8()?;
    let _flags = c.u32()?;
    let attr_count = c.i32()?;
    if !(0..=256).contains(&attr_count) {
        return Err(Error::BadStructure("implausible attribute count".into()));
    }
    for _ in 0..attr_count {
        c.i32()?;
    }
    // Group Node Data tail: U8 version + I32 child count + child ids
    let _gver = c.u8()?;
    let child_count = c.i32()?;
    if !(0..=1_000_000).contains(&child_count) {
        return Err(Error::BadStructure("implausible child count".into()));
    }
    let mut children = Vec::with_capacity(child_count as usize);
    for _ in 0..child_count {
        children.push(c.i32()?);
    }
    Ok(children)
}

/// Decode the LSG block into object tables. `block` must be the LSG entry
/// whose data begins at `block.offset + 24`.
pub(crate) fn parse_lsg(bytes: &[u8], block: &BlockEntry, big: bool) -> Result<LsgFacts, Error> {
    let start = block.offset + 24;
    let end = start + block.length - 24;
    if end > bytes.len() {
        return Err(Error::BadStructure("LSG block overruns file".into()));
    }
    let mut c = Cursor::new(bytes, big);
    c.pos = start;

    let mut elements = Vec::new();
    let mut first = true;
    let guard_end = end;
    loop {
        if c.pos >= guard_end {
            break;
        }
        match read_compressed_element(&mut c, first) {
            Ok(Some(el)) => {
                first = false;
                elements.push(el);
            }
            Ok(None) => break,
            Err(e) => return Err(e),
        }
    }

    let mut facts = LsgFacts {
        parts: HashMap::new(),
        names: HashMap::new(),
        late_load: HashMap::new(),
        props_of: HashMap::new(),
        root_children: Vec::new(),
        warnings: Vec::new(),
    };

    for el in &elements {
        if guid_eq(&el.type_id, &GID_PARTITION) {
            facts.root_children = group_children(&el.data, big)?;
        } else if guid_eq(&el.type_id, &GID_PART) {
            let children = group_children(&el.data, big)?;
            facts.parts.insert(el.object_id, (children, None));
        } else if guid_eq(&el.type_id, &GID_STRING_PROP) && el.base_type == BASE_PROPERTY {
            match decode_string_prop(&el.data, big) {
                Some(s) => {
                    facts.names.insert(el.object_id, s);
                }
                None => facts
                    .warnings
                    .push("string property atom with undecodable value".into()),
            }
        } else if guid_eq(&el.type_id, &GID_LATE_LOAD) && el.base_type == BASE_PROPERTY {
            if let Some(t) = decode_late_load(&el.data, big) {
                facts.late_load.insert(el.object_id, t);
            }
        } else if guid_eq(&el.type_id, &GID_INSTANCE)
            || guid_eq(&el.type_id, &GID_GROUP)
            || guid_eq(&el.type_id, &GID_LOD)
            || guid_eq(&el.type_id, &GID_SWITCH)
        {
            facts
                .warnings
                .push("recognized-but-unmapped node element skipped".into());
        } else {
            facts
                .warnings
                .push("unknown element type skipped (tolerant)".into());
        }
    }

    parse_property_table(&mut c, big, &mut facts)?;
    Ok(facts)
}

/// Decode String Property Atom payload: Base Property Atom Data (U8 version +
/// U32 state flags) + U8 version + MbString value (Ref 6.2.2).
fn decode_string_prop(data: &[u8], big: bool) -> Option<String> {
    let mut c = Cursor::new(data, big);
    let _ver = c.u8().ok()?;
    let _flags = c.u32().ok()?;
    let _ver2 = c.u8().ok()?;
    let count = c.i32().ok()?;
    if !(0..=1_000_000).contains(&count) {
        return None;
    }
    let raw = c.take((count as usize).saturating_mul(2)).ok()?;
    let (chunks, _rest): (&[[u8; 2]], &[u8]) = raw.as_chunks::<2>();
    let units: Vec<u16> = chunks.iter().map(|b| u16::from_le_bytes(*b)).collect();
    String::from_utf16(&units).ok()
}

/// Decode Late Loaded Property Atom payload: U8 version + GUID block id +
/// I32 block kind + I32 payload object id + I32 reserved (Ref 6.2.7).
fn decode_late_load(data: &[u8], big: bool) -> Option<([u8; 16], u32, i32)> {
    let mut c = Cursor::new(data, big);
    let _ver = c.u8().ok()?;
    let mut g = [0u8; 16];
    g.copy_from_slice(c.take(16).ok()?);
    let kind = c.i32().ok()? as u32;
    let payload = c.i32().ok()?;
    Some((g, kind, payload))
}

/// Parse the Property Table (Ref 6.3): I16 version + I32 count then per
/// element: I32 element oid + (I32 key, I32 value) pairs until key == 0.
fn parse_property_table(c: &mut Cursor, _big: bool, facts: &mut LsgFacts) -> Result<(), Error> {
    if c.remaining() < 2 {
        return Ok(());
    }
    let _ver = c.u16()?;
    let count = c.i32()?;
    if !(0..=1_000_000).contains(&count) {
        return Err(Error::BadStructure(
            "implausible property table count".into(),
        ));
    }
    for _ in 0..count {
        let element_oid = c.i32()?;
        let mut values = Vec::new();
        loop {
            let key = c.i32()?;
            if key == 0 {
                break;
            }
            let value = c.i32()?;
            values.push(value);
        }
        facts.props_of.insert(element_oid, values);
    }
    Ok(())
}

/// Resolve the LSG into vendor-neutral parts, decoding shape references.
pub(crate) fn resolve_parts(
    facts: &LsgFacts,
    bytes: &[u8],
    blocks: &[BlockEntry],
    big: bool,
) -> Result<(Vec<PartNode>, Vec<String>), Error> {
    let mut parts = Vec::new();
    let mut warnings = facts.warnings.clone();

    let add_part = |oid: i32,
                    children: &[i32],
                    bbox: Option<BoundingBox>,
                    facts: &LsgFacts,
                    blocks: &[BlockEntry],
                    bytes: &[u8],
                    big: bool,
                    warnings: &mut Vec<String>|
     -> Result<PartNode, Error> {
        let mut name = String::from("unnamed-part");
        let mut shape_ref: Option<([u8; 16], u32, i32)> = None;
        if let Some(values) = facts.props_of.get(&oid) {
            for &v in values {
                if let Some(s) = facts.names.get(&v) {
                    name = s.clone();
                }
                if let Some(t) = facts.late_load.get(&v) {
                    shape_ref = Some(*t);
                }
            }
        }
        let mut part = PartNode {
            object_id: oid,
            name,
            bounding: bbox,
            mesh: None,
            material: None,
        };
        if let Some((block_guid, block_kind, payload_id)) = shape_ref {
            match resolve_shape_reference(bytes, blocks, &block_guid, block_kind, payload_id, big) {
                Ok(mesh) => part.mesh = Some(mesh),
                Err(Error::UnsupportedFeature(m)) => warnings.push(m),
                Err(e) => return Err(e),
            }
        }
        if !children.is_empty() {
            warnings.push(format!(
                "part node {oid}: {} child reference(s) not expanded (wedge keeps part leaf)",
                children.len()
            ));
        }
        Ok(part)
    };

    for child_id in &facts.root_children {
        if let Some((children, bbox)) = facts.parts.get(child_id) {
            parts.push(add_part(
                *child_id,
                children,
                *bbox,
                facts,
                blocks,
                bytes,
                big,
                &mut warnings,
            )?);
        } else {
            warnings.push(format!("partition child id {child_id} not found in LSG"));
        }
    }

    // Tolerant: also surface part nodes not reachable from the partition root
    // so partial scene graphs still yield their geometry.
    for (oid, (children, bbox)) in &facts.parts {
        if parts.iter().all(|p| p.object_id != *oid) {
            parts.push(add_part(
                *oid,
                children,
                *bbox,
                facts,
                blocks,
                bytes,
                big,
                &mut warnings,
            )?);
        }
    }
    Ok((parts, warnings))
}

/// Find the shape block by GUID and decode the element whose object id equals
/// the late-loaded payload id into a facet mesh.
fn resolve_shape_reference(
    bytes: &[u8],
    blocks: &[BlockEntry],
    block_guid: &[u8; 16],
    block_kind: u32,
    payload_id: i32,
    big: bool,
) -> Result<FacetMesh, Error> {
    if !(6..=16).contains(&block_kind) {
        return Err(Error::UnsupportedFeature(format!(
            "shape reference kind {block_kind} is not a shape representation this wedge decodes"
        )));
    }
    let block = blocks
        .iter()
        .find(|b| &b.guid == block_guid)
        .ok_or_else(|| {
            Error::BadStructure("shape block reference not found in directory".into())
        })?;
    let mut c = Cursor::new(bytes, big);
    c.pos = block.offset + 24;
    let end = block.offset + block.length;
    loop {
        if c.pos >= end {
            break;
        }
        match read_plain_element(&mut c)? {
            Some(el) => {
                if el.object_id == payload_id {
                    return super::shape::decode_shape_element(&el, big);
                }
            }
            None => break,
        }
    }
    Err(Error::BadStructure(format!(
        "shape block has no element with object id {payload_id}"
    )))
}

/// Provide the element struct to the shape module.
pub(crate) use Element as RawElement;

impl Element {
    /// Borrow accessor used by the shape decoder.
    pub(crate) fn data(&self) -> &[u8] {
        &self.data
    }
    pub(crate) fn type_id(&self) -> &[u8; 16] {
        &self.type_id
    }
}
