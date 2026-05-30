//! Phase 3.7：Paradox `.mesh` 二进制格式解析器（typed binary tree 重写）。
//!
//! 替换 Phase 2.7 的启发式扫描——经过对 `gfx/models/mapitems/trees/*.mesh`
//! 的二进制逆向（详见 ROADMAP_V3 §3.7.1），HOI4 mesh 文件是一棵 typed
//! binary tree，节点头由开标记字节驱动：
//!
//! * `!` (0x21)  — **属性叶子**（attribute leaf）
//!   - `name_len:u8`
//!   - `name:[u8; name_len]`
//!   - `type_byte:u8` — `i` int / `f` float / `s` string
//!   - `count:u32` — 元素数量（int / float = 个数；string = 字符串个数）
//!   - payload：
//!     - `i` → `count × u32`
//!     - `f` → `count × f32`
//!     - `s` → 重复 `[len:u32][bytes(len, 末位 0x00)]` `count` 次
//! * `[` (0x5B) — **对象容器**（object container），可能 1~4 个连写表示绝对深度
//!   - 连续的 `[` 字节数 = 该对象的绝对深度 N
//!   - 接着 `name:cstring`（null 终止）
//!   - 子节点继续按本规则解析；遇到下一个 `[` 链时，深度变化指示出栈/入栈
//!
//! 入口树形（vanilla 树木 mesh 实测，beech.mesh）：
//! ```text
//! pdxasset (i, count=2)
//! [object
//!   loddist (f, count=N)         — 各 LOD 切换距离
//!   [[lodShape0 / loddtvaShape   — 一个 LOD 容器（名字因模型而异）
//!     lod (i, count=1)            — LOD 序号
//!     [[[mesh                     — 顶点/索引数据
//!       p   (f, 3·V)              — 顶点位置
//!       n   (f, 3·V)              — 法线
//!       ta  (f, 4·V)              — 切线（可选）
//!       u0  (f, 2·V)              — UV 通道 0
//!       u1  (f, 2·V)              — UV 通道 1（可选）
//!       tri (i, 3·T)              — u32 索引（每三角形 3 个）
//!     [[[[aabb                    — 包围盒
//!       min/max (f, 3)
//!     [[[[material
//!       shader (s) / diff (s) / n (s) / spec (s)
//!   [[lodShape1 …                 — 下一个 LOD 容器
//! ```
//!
//! 解析器策略：
//! * 维护一个深度栈 `Vec<NodeName>`。`[`链长度 = 栈深度（含本节点）。读到 `[`链时
//!   先 `truncate` 到 `bracket_count - 1`，再 push 新节点。
//! * 当栈 `[..., loddt*Shape, mesh]` 时收集 mesh 属性；当栈 `[..., aabb]` 时收集
//!   bounds；当栈 `[..., material]` 时收集材质字符串。
//! * 同一个 LOD 容器的 `mesh` + `aabb` + `material` 构成一个 [`SubMesh`]；
//!   遇到下一个 `[[lodShape*` 时 flush。
//!
//! 错误兼容：未知字段静默忽略（mod / 未来 DLC 扩展），但破坏顶部 magic 或读越界
//! 直接报错。

use crate::error::AssetError;

/// 解析后的 Paradox mesh。每个 LOD（或独立 shape）一个 [`SubMesh`]。
#[derive(Debug, Clone)]
pub struct PdxMesh {
    /// 全局包围盒（取所有 submesh 包围盒并集；若无 submesh 则全 0）。
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
    /// LOD 切换距离阈值（来自顶层 `loddist` 属性）。
    pub lod_distances: Vec<f32>,
    /// 每个 LOD / 形状一个 SubMesh。
    pub meshes: Vec<SubMesh>,
}

/// 单个 mesh draw call 的数据。
#[derive(Debug, Clone)]
pub struct SubMesh {
    /// 该 LOD 的序号（来自容器内 `lod` 属性）。
    pub lod: u32,
    /// 容器名（如 `loddtvaShape` / `lodShape0`），用于调试/识别。
    pub shape_name: String,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    /// UV 通道 1（可选；常用于 lightmap / detail）。
    pub uvs1: Vec<[f32; 2]>,
    pub tangents: Vec<[f32; 4]>,
    pub bone_indices: Vec<[u8; 4]>,
    pub bone_weights: Vec<[f32; 4]>,
    /// u32 索引（Paradox 用 32-bit 索引，与 GPU 通用）。
    pub indices: Vec<u32>,
    /// 子 mesh 包围盒（来自同级 `aabb` 块）。
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
    pub material: MeshMaterial,
}

/// 材质引用。
#[derive(Debug, Clone, Default)]
pub struct MeshMaterial {
    pub shader: String,
    pub diffuse: Option<String>,
    pub normal: Option<String>,
    pub specular: Option<String>,
}

/// 顶层魔数（HOI4 / EU4 / CK3 共用）。
const MESH_MAGIC: &[u8; 4] = b"@@b@";

impl PdxMesh {
    /// 从字节解析。供 [`crate::AssetDb::parse_or_get`] 使用。
    pub fn parse(bytes: &[u8]) -> Result<Self, AssetError> {
        parse_pdx_mesh(bytes)
    }

    pub fn vertex_count(&self) -> usize {
        self.meshes.iter().map(|m| m.positions.len()).sum()
    }

    pub fn index_count(&self) -> usize {
        self.meshes.iter().map(|m| m.indices.len()).sum()
    }

    /// 选择给定 LOD（按 `loddist` 阈值）。当 LOD 索引超出可用 mesh 时返回最后一个。
    pub fn pick_lod(&self, lod_index: usize) -> Option<&SubMesh> {
        if self.meshes.is_empty() {
            return None;
        }
        let idx = lod_index.min(self.meshes.len() - 1);
        Some(&self.meshes[idx])
    }
}

// ─── Parser core ────────────────────────────────────────────────────────────

fn parse_pdx_mesh(bytes: &[u8]) -> Result<PdxMesh, AssetError> {
    if bytes.len() < 4 {
        return Err(AssetError::parse("", "mesh too small (< 4 bytes)"));
    }
    if &bytes[0..4] != MESH_MAGIC {
        return Err(AssetError::parse(
            "",
            format!("bad mesh magic: {:02X?}", &bytes[0..4]),
        ));
    }

    let mut cursor = Cursor::new(&bytes[4..]);
    let mut state = ParserState::default();

    while !cursor.eof() {
        let marker = cursor.peek_u8()?;
        match marker {
            b'!' => {
                cursor.advance(1);
                parse_attribute(&mut cursor, &mut state)?;
            }
            b'[' => {
                parse_container(&mut cursor, &mut state)?;
            }
            // 兼容性：若文件以零字节结尾或有未知 trailing 数据，宽容跳过。
            _ => {
                cursor.advance(1);
            }
        }
    }

    state.flush_current_shape();

    // 计算全局 bounds。
    let mut gmin = [f32::MAX; 3];
    let mut gmax = [f32::MIN; 3];
    let mut any = false;
    for sub in &state.meshes {
        for p in &sub.positions {
            for i in 0..3 {
                gmin[i] = gmin[i].min(p[i]);
                gmax[i] = gmax[i].max(p[i]);
            }
            any = true;
        }
    }
    if !any {
        gmin = [0.0; 3];
        gmax = [0.0; 3];
    }

    Ok(PdxMesh {
        bounds_min: gmin,
        bounds_max: gmax,
        lod_distances: state.lod_distances,
        meshes: state.meshes,
    })
}

#[derive(Default)]
struct ParserState {
    /// 当前节点路径栈（按深度索引，0=root 不入栈，第一层"object"对应 stack[0]）。
    stack: Vec<String>,
    /// 顶层 `loddist` 数据（来自 `[object` 内的 `loddist` 属性）。
    lod_distances: Vec<f32>,
    /// 已收齐的 SubMesh。
    meshes: Vec<SubMesh>,
    /// 当前正在收集的 SubMesh（在 `[[lodShape*` 容器内）。
    current: Option<SubMesh>,
}

impl ParserState {
    /// 当前栈是否处在一个"shape"容器内（深度 2 的容器，object 的直接子节点）。
    /// 我们对名字不挑剔——除了"loddtvaShape / lodShape0 / loddtreShape"这些
    /// HOI4 树木用的，建筑模型用 `polySurface***Shape` / `pCubeShape*` 等任意
    /// 命名，所以策略改成"凡是 object 下的子容器都视作 shape 候选"。
    /// 单元 / 飞机模型也走同一路径。
    fn in_shape_container(&self) -> Option<&str> {
        if self.stack.len() >= 2 {
            return Some(self.stack[1].as_str());
        }
        None
    }

    fn in_mesh(&self) -> bool {
        self.stack.last().map(|s| s == "mesh").unwrap_or(false)
    }

    fn in_aabb(&self) -> bool {
        self.stack.last().map(|s| s == "aabb").unwrap_or(false)
    }

    fn in_material(&self) -> bool {
        self.stack.last().map(|s| s == "material").unwrap_or(false)
    }

    /// 离开当前 shape 时，把 `current` 推到 `meshes` 列表。仅当顶点 / 索引非空
    /// 才保留——空 shape（如 skeleton / collision-only 容器）被丢弃。
    fn flush_current_shape(&mut self) {
        if let Some(sub) = self.current.take() {
            if !sub.positions.is_empty() && !sub.indices.is_empty() {
                self.meshes.push(sub);
            }
        }
    }

    fn enter_shape(&mut self, name: &str) {
        self.flush_current_shape();
        self.current = Some(SubMesh {
            lod: 0,
            shape_name: name.to_string(),
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            uvs1: Vec::new(),
            tangents: Vec::new(),
            bone_indices: Vec::new(),
            bone_weights: Vec::new(),
            indices: Vec::new(),
            bounds_min: [0.0; 3],
            bounds_max: [0.0; 3],
            material: MeshMaterial::default(),
        });
    }
}

/// 仅用于测试 / 兼容性诊断：是否符合"经典 LOD 形状"命名（HOI4 树木 / EU4 单位
/// 用过几种命名前缀）。解析器 *不* 用此函数过滤——见 `enter_shape` 注释。
#[cfg(test)]
fn is_shape_container(name: &str) -> bool {
    name.starts_with("lodShape")
        || (name.starts_with("loddt") && name.ends_with("Shape"))
        || (name.starts_with("lod") && name.ends_with("Shape"))
}

/// 解析 `[`链：连续的 `[` 字节数 = 新对象的绝对深度。
fn parse_container(cursor: &mut Cursor, state: &mut ParserState) -> Result<(), AssetError> {
    let mut depth = 0usize;
    while !cursor.eof() && cursor.peek_u8()? == b'[' {
        cursor.advance(1);
        depth += 1;
        if depth > 16 {
            return Err(AssetError::parse("", "mesh container depth > 16"));
        }
    }
    let name = cursor.read_cstring()?;
    if depth == 0 {
        return Err(AssetError::parse("", "container with zero brackets"));
    }
    // 出栈：保留 depth - 1 层，然后 push 当前。
    // 离开 / 切换 depth-2 shape 时，把当前 SubMesh flush 到列表。
    if depth - 1 < state.stack.len() {
        // 当前栈深度 ≥ 2 表示我们正在某个 shape 内；新节点 depth ≤ 2 意味着
        // 要么进入兄弟 shape（depth==2），要么退到 root（depth==1，不会出现）。
        // 这两种情况都需要 flush。
        if state.stack.len() >= 2 && depth <= 2 {
            state.flush_current_shape();
        }
        state.stack.truncate(depth - 1);
    }
    state.stack.push(name.clone());

    // 进入新 shape（任意 depth-2 容器）时初始化 current。
    if depth == 2 {
        state.enter_shape(&name);
    }

    Ok(())
}

/// 解析 `!` 属性：name + type + count + payload。
fn parse_attribute(cursor: &mut Cursor, state: &mut ParserState) -> Result<(), AssetError> {
    let name_len = cursor.read_u8()? as usize;
    if name_len == 0 || name_len > 64 {
        return Err(AssetError::parse(
            "",
            format!("bad attribute name length: {}", name_len),
        ));
    }
    let name_bytes = cursor.read_bytes(name_len)?;
    let name = String::from_utf8_lossy(name_bytes).to_string();
    let type_byte = cursor.read_u8()?;
    let count = cursor.read_u32_le()? as usize;

    // 上限保护：单个数组最大 ~50 M 元素，避免恶意/损坏文件吃光内存。
    if count > 50_000_000 {
        return Err(AssetError::parse(
            "",
            format!("attribute '{}' count too large: {}", name, count),
        ));
    }

    match type_byte {
        b'i' => {
            // u32 数组
            let mut data = Vec::with_capacity(count);
            for _ in 0..count {
                data.push(cursor.read_u32_le()?);
            }
            apply_int_attr(state, &name, &data);
        }
        b'f' => {
            let mut data = Vec::with_capacity(count);
            for _ in 0..count {
                data.push(cursor.read_f32_le()?);
            }
            apply_float_attr(state, &name, &data);
        }
        b's' => {
            let mut strings = Vec::with_capacity(count);
            for _ in 0..count {
                let slen = cursor.read_u32_le()? as usize;
                if slen > 4096 {
                    return Err(AssetError::parse(
                        "",
                        format!("attribute '{}' string len too large: {}", name, slen),
                    ));
                }
                let bytes = cursor.read_bytes(slen)?;
                // 末尾通常带 0x00；剥离。
                let trimmed: &[u8] = if let Some(&last) = bytes.last() {
                    if last == 0 {
                        &bytes[..bytes.len() - 1]
                    } else {
                        bytes
                    }
                } else {
                    bytes
                };
                strings.push(String::from_utf8_lossy(trimmed).to_string());
            }
            apply_string_attr(state, &name, &strings);
        }
        // 未知类型：保守跳过。我们没有元素大小信息，只能依赖 count 表示字节数。
        // 这个分支 vanilla 不会触发；放在这是为了不在未来 DLC 扩展时整文件解析失败。
        _ => {
            // 跳过 count 个字节作为最坏估计（不一定正确，但最起码不无限循环）。
            cursor.advance(count);
        }
    }

    Ok(())
}

fn apply_int_attr(state: &mut ParserState, name: &str, data: &[u32]) {
    if state.in_mesh() {
        if name == "tri" {
            if let Some(sub) = state.current.as_mut() {
                sub.indices.extend_from_slice(data);
            }
        } else if name == "bi" {
            // bone indices: count = 4·V, 但 vanilla tree mesh 没有；建筑/单位会有。
            if let Some(sub) = state.current.as_mut() {
                let v = data.len() / 4;
                for i in 0..v {
                    let base = i * 4;
                    let raw = [
                        (data[base] & 0xFF) as u8,
                        (data[base + 1] & 0xFF) as u8,
                        (data[base + 2] & 0xFF) as u8,
                        (data[base + 3] & 0xFF) as u8,
                    ];
                    sub.bone_indices.push(raw);
                }
            }
        }
        return;
    }
    if state.in_shape_container().is_some() {
        if name == "lod" && !data.is_empty() {
            if let Some(sub) = state.current.as_mut() {
                sub.lod = data[0];
            }
        }
    }
}

fn apply_float_attr(state: &mut ParserState, name: &str, data: &[f32]) {
    // 顶层 loddist：栈[object]
    if state.stack.len() == 1 && state.stack[0] == "object" && name == "loddist" {
        state.lod_distances.clear();
        state.lod_distances.extend_from_slice(data);
        return;
    }

    if state.in_mesh() {
        let Some(sub) = state.current.as_mut() else {
            return;
        };
        match name {
            "p" => fill_vec3(&mut sub.positions, data),
            "n" => fill_vec3(&mut sub.normals, data),
            "ta" => fill_vec4(&mut sub.tangents, data),
            "u0" => fill_vec2(&mut sub.uvs, data),
            "u1" => fill_vec2(&mut sub.uvs1, data),
            "bw" => fill_vec4(&mut sub.bone_weights, data),
            _ => {} // 未知属性静默忽略
        }
        return;
    }

    if state.in_aabb() {
        let Some(sub) = state.current.as_mut() else {
            return;
        };
        if data.len() >= 3 {
            match name {
                "min" => sub.bounds_min = [data[0], data[1], data[2]],
                "max" => sub.bounds_max = [data[0], data[1], data[2]],
                _ => {}
            }
        }
        return;
    }
}

fn apply_string_attr(state: &mut ParserState, name: &str, strings: &[String]) {
    if !state.in_material() {
        return;
    }
    let Some(sub) = state.current.as_mut() else {
        return;
    };
    // material 内的字符串属性都只取第一个值。
    let first = strings.first().cloned().unwrap_or_default();
    match name {
        "shader" => sub.material.shader = first,
        "diff" => sub.material.diffuse = Some(first),
        "n" => sub.material.normal = Some(first),
        "spec" => sub.material.specular = Some(first),
        _ => {} // ext slots: light_diff, terrain_a 等未来可加
    }
}

fn fill_vec3(out: &mut Vec<[f32; 3]>, data: &[f32]) {
    let n = data.len() / 3;
    out.reserve(n);
    for i in 0..n {
        let b = i * 3;
        out.push([data[b], data[b + 1], data[b + 2]]);
    }
}

fn fill_vec4(out: &mut Vec<[f32; 4]>, data: &[f32]) {
    let n = data.len() / 4;
    out.reserve(n);
    for i in 0..n {
        let b = i * 4;
        out.push([data[b], data[b + 1], data[b + 2], data[b + 3]]);
    }
}

fn fill_vec2(out: &mut Vec<[f32; 2]>, data: &[f32]) {
    let n = data.len() / 2;
    out.reserve(n);
    for i in 0..n {
        let b = i * 2;
        out.push([data[b], data[b + 1]]);
    }
}

// ─── Cursor helper ──────────────────────────────────────────────────────────

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn eof(&self) -> bool {
        self.pos >= self.data.len()
    }

    fn advance(&mut self, n: usize) {
        self.pos = self.pos.saturating_add(n).min(self.data.len());
    }

    fn peek_u8(&self) -> Result<u8, AssetError> {
        self.data
            .get(self.pos)
            .copied()
            .ok_or_else(|| AssetError::parse("", "unexpected EOF (peek)"))
    }

    fn read_u8(&mut self) -> Result<u8, AssetError> {
        let b = self.peek_u8()?;
        self.pos += 1;
        Ok(b)
    }

    fn read_u32_le(&mut self) -> Result<u32, AssetError> {
        if self.pos + 4 > self.data.len() {
            return Err(AssetError::parse("", "unexpected EOF reading u32"));
        }
        let v = u32::from_le_bytes(self.data[self.pos..self.pos + 4].try_into().unwrap());
        self.pos += 4;
        Ok(v)
    }

    fn read_f32_le(&mut self) -> Result<f32, AssetError> {
        if self.pos + 4 > self.data.len() {
            return Err(AssetError::parse("", "unexpected EOF reading f32"));
        }
        let v = f32::from_le_bytes(self.data[self.pos..self.pos + 4].try_into().unwrap());
        self.pos += 4;
        Ok(v)
    }

    fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], AssetError> {
        if self.pos + n > self.data.len() {
            return Err(AssetError::parse(
                "",
                format!("unexpected EOF reading {} bytes", n),
            ));
        }
        let slice = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    /// 读 null 终止字符串（消耗 0x00）。
    fn read_cstring(&mut self) -> Result<String, AssetError> {
        let start = self.pos;
        while !self.eof() {
            if self.data[self.pos] == 0 {
                let s = String::from_utf8_lossy(&self.data[start..self.pos]).to_string();
                self.pos += 1; // 跳过 NUL
                return Ok(s);
            }
            self.pos += 1;
            if self.pos - start > 256 {
                return Err(AssetError::parse("", "cstring too long (>256)"));
            }
        }
        Err(AssetError::parse("", "unterminated cstring"))
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个最小合法 mesh：
    /// `@@b@`
    /// `! pdxasset i 2 [1, 0]`
    /// `[ object`
    /// `! loddist f 1 [0]`
    /// `[[ lodShape0`
    /// `! lod i 1 [0]`
    /// `[[[ mesh`
    /// `! p f 6 [pos×2]`
    /// `! n f 6 [normal×2]`
    /// `! u0 f 4 [uv×2]`
    /// `! tri i 3 [0,1,0]`
    /// `[[[[ aabb`
    /// `! min f 3 [-1,-1,-1]`
    /// `! max f 3 [1,1,1]`
    /// `[[[[ material`
    /// `! shader s 1 ["TestShader\0"]`
    /// `! diff   s 1 ["test_diffuse.dds\0"]`
    fn build_synthetic_mesh() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(MESH_MAGIC);

        // ! pdxasset i 2 [1, 0]
        data.push(b'!');
        data.push(8);
        data.extend_from_slice(b"pdxasset");
        data.push(b'i');
        data.extend_from_slice(&2u32.to_le_bytes());
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());

        // [ object\0
        data.push(b'[');
        data.extend_from_slice(b"object\0");

        // ! loddist f 1 [0.0]
        data.push(b'!');
        data.push(7);
        data.extend_from_slice(b"loddist");
        data.push(b'f');
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&0.0f32.to_le_bytes());

        // [[ lodShape0\0
        data.push(b'[');
        data.push(b'[');
        data.extend_from_slice(b"lodShape0\0");

        // ! lod i 1 [0]
        data.push(b'!');
        data.push(3);
        data.extend_from_slice(b"lod");
        data.push(b'i');
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());

        // [[[ mesh\0
        data.push(b'[');
        data.push(b'[');
        data.push(b'[');
        data.extend_from_slice(b"mesh\0");

        // ! p f 6 [(1,2,3),(4,5,6)]
        data.push(b'!');
        data.push(1);
        data.extend_from_slice(b"p");
        data.push(b'f');
        data.extend_from_slice(&6u32.to_le_bytes());
        for &x in &[1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0] {
            data.extend_from_slice(&x.to_le_bytes());
        }

        // ! n f 6
        data.push(b'!');
        data.push(1);
        data.extend_from_slice(b"n");
        data.push(b'f');
        data.extend_from_slice(&6u32.to_le_bytes());
        for &x in &[0.0f32, 1.0, 0.0, 0.0, 0.0, 1.0] {
            data.extend_from_slice(&x.to_le_bytes());
        }

        // ! u0 f 4
        data.push(b'!');
        data.push(2);
        data.extend_from_slice(b"u0");
        data.push(b'f');
        data.extend_from_slice(&4u32.to_le_bytes());
        for &x in &[0.5f32, 0.5, 1.0, 0.0] {
            data.extend_from_slice(&x.to_le_bytes());
        }

        // ! tri i 3 [0,1,0]
        data.push(b'!');
        data.push(3);
        data.extend_from_slice(b"tri");
        data.push(b'i');
        data.extend_from_slice(&3u32.to_le_bytes());
        for &x in &[0u32, 1, 0] {
            data.extend_from_slice(&x.to_le_bytes());
        }

        // [[[[ aabb\0
        for _ in 0..4 {
            data.push(b'[');
        }
        data.extend_from_slice(b"aabb\0");

        // ! min f 3
        data.push(b'!');
        data.push(3);
        data.extend_from_slice(b"min");
        data.push(b'f');
        data.extend_from_slice(&3u32.to_le_bytes());
        for &x in &[-1.0f32, -1.0, -1.0] {
            data.extend_from_slice(&x.to_le_bytes());
        }
        // ! max f 3
        data.push(b'!');
        data.push(3);
        data.extend_from_slice(b"max");
        data.push(b'f');
        data.extend_from_slice(&3u32.to_le_bytes());
        for &x in &[1.0f32, 1.0, 1.0] {
            data.extend_from_slice(&x.to_le_bytes());
        }

        // [[[[ material\0
        for _ in 0..4 {
            data.push(b'[');
        }
        data.extend_from_slice(b"material\0");

        // ! shader s 1 ["TestShader\0"] (len=11)
        data.push(b'!');
        data.push(6);
        data.extend_from_slice(b"shader");
        data.push(b's');
        data.extend_from_slice(&1u32.to_le_bytes());
        let shader = b"TestShader\0";
        data.extend_from_slice(&(shader.len() as u32).to_le_bytes());
        data.extend_from_slice(shader);

        // ! diff s 1 ["test_diffuse.dds\0"]
        data.push(b'!');
        data.push(4);
        data.extend_from_slice(b"diff");
        data.push(b's');
        data.extend_from_slice(&1u32.to_le_bytes());
        let diff = b"test_diffuse.dds\0";
        data.extend_from_slice(&(diff.len() as u32).to_le_bytes());
        data.extend_from_slice(diff);

        data
    }

    #[test]
    fn rejects_bad_magic() {
        let data = vec![0u8; 64];
        assert!(PdxMesh::parse(&data).is_err());
    }

    #[test]
    fn rejects_too_small() {
        assert!(PdxMesh::parse(&[0u8; 3]).is_err());
    }

    #[test]
    fn parses_synthetic_mesh() {
        let bytes = build_synthetic_mesh();
        let mesh = PdxMesh::parse(&bytes).expect("synthetic mesh parses");
        assert_eq!(mesh.lod_distances, vec![0.0]);
        assert_eq!(mesh.meshes.len(), 1);
        let sub = &mesh.meshes[0];
        assert_eq!(sub.shape_name, "lodShape0");
        assert_eq!(sub.lod, 0);
        assert_eq!(sub.positions, vec![[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]);
        assert_eq!(sub.normals.len(), 2);
        assert_eq!(sub.uvs, vec![[0.5, 0.5], [1.0, 0.0]]);
        assert_eq!(sub.indices, vec![0, 1, 0]);
        assert_eq!(sub.bounds_min, [-1.0, -1.0, -1.0]);
        assert_eq!(sub.bounds_max, [1.0, 1.0, 1.0]);
        assert_eq!(sub.material.shader, "TestShader");
        assert_eq!(sub.material.diffuse.as_deref(), Some("test_diffuse.dds"));
    }

    #[test]
    fn global_bounds_match_submesh() {
        let bytes = build_synthetic_mesh();
        let mesh = PdxMesh::parse(&bytes).unwrap();
        // 顶点 (1,2,3) 与 (4,5,6) → bounds [1,2,3] → [4,5,6]
        assert_eq!(mesh.bounds_min, [1.0, 2.0, 3.0]);
        assert_eq!(mesh.bounds_max, [4.0, 5.0, 6.0]);
    }

    #[test]
    fn empty_mesh_when_no_data() {
        let mut data = Vec::new();
        data.extend_from_slice(MESH_MAGIC);
        let m = PdxMesh::parse(&data).unwrap();
        assert!(m.meshes.is_empty());
        assert_eq!(m.bounds_min, [0.0; 3]);
    }

    #[test]
    fn shape_container_detection() {
        assert!(is_shape_container("lodShape0"));
        assert!(is_shape_container("lodShape42"));
        assert!(is_shape_container("loddtvaShape"));
        assert!(is_shape_container("loddtreShape"));
        assert!(!is_shape_container("mesh"));
        assert!(!is_shape_container("aabb"));
        assert!(!is_shape_container("material"));
    }

    #[test]
    fn pick_lod_clamps_to_last() {
        let bytes = build_synthetic_mesh();
        let mesh = PdxMesh::parse(&bytes).unwrap();
        assert!(mesh.pick_lod(0).is_some());
        assert!(mesh.pick_lod(99).is_some()); // 越界 clamp 到最后一个
    }

    #[test]
    fn rejects_huge_attribute_count() {
        // 构造 magic + ! a i 99_999_999 …
        let mut data = Vec::new();
        data.extend_from_slice(MESH_MAGIC);
        data.push(b'!');
        data.push(1);
        data.extend_from_slice(b"x");
        data.push(b'i');
        data.extend_from_slice(&60_000_000u32.to_le_bytes());
        let r = PdxMesh::parse(&data);
        assert!(r.is_err());
    }
}
