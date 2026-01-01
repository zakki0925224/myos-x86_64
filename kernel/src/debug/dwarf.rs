use crate::{
    error::{Error, Result},
    util,
};
use alloc::{collections::BTreeMap, string::String, vec::Vec};
use common::elf::Elf64;

// https://dwarfstd.org/doc/DWARF5.pdf

// 7.5.1 Unit Headers
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnitType {
    Compile,
    Type,
    Partial,
    Skeleton,
    SplitCompile,
    SplitType,
    User(u8),
}

impl TryFrom<u8> for UnitType {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0x01 => Ok(Self::Compile),
            0x02 => Ok(Self::Type),
            0x03 => Ok(Self::Partial),
            0x04 => Ok(Self::Skeleton),
            0x05 => Ok(Self::SplitCompile),
            0x06 => Ok(Self::SplitType),
            0x80..=0xff => Ok(Self::User(value)),
            _ => Err("Invalid UnitType value".into()),
        }
    }
}

// 7.5.1.1 Full and Partial Compilation Unit Headers
#[derive(Clone)]
pub struct DebugInfo {
    pub unit_length: u32,
    pub version: u16,
    pub unit_type: UnitType,
    pub address_size: u8,
    pub debug_abbrev_offset: u32,
    pub dwo_id: Option<u64>, // 7.5.1.2 Skeleton and Split Compilation Unit Headers
    pub type_sig: Option<u64>, // 7.5.1.3 Type Unit Headers
    pub type_offset: Option<u64>,
    data: Vec<u8>,
}

impl core::fmt::Debug for DebugInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DebugInfo")
            .field("unit_length", &self.unit_length)
            .field("version", &self.version)
            .field("unit_type", &self.unit_type)
            .field("address_size", &self.address_size)
            .field("debug_abbrev_offset", &self.debug_abbrev_offset)
            .field("dwo_id", &self.dwo_id)
            .field("type_sig", &self.type_sig)
            .field("type_offset", &self.type_offset)
            .finish()
    }
}

impl TryFrom<&[u8]> for DebugInfo {
    type Error = Error;

    fn try_from(value: &[u8]) -> Result<Self> {
        if value.len() < 4 {
            return Err("Invalid DebugInfo length (unit_length)".into());
        }

        let unit_length = u32::from_le_bytes([value[0], value[1], value[2], value[3]]);

        if unit_length == 0xffff_ffff {
            return Err("64-bit DWARF format is not supported".into());
        }

        let total_unit_size = 4 + unit_length as usize;
        if value.len() < total_unit_size {
            return Err("DebugInfo data section out of bounds (unit_length mismatch)".into());
        }

        if value.len() < 12 {
            return Err("Invalid DebugInfo length (header minimum)".into());
        }

        let version = u16::from_le_bytes([value[4], value[5]]);
        if version != 5 {
            return Err("Unsupported DWARF version".into());
        }

        let unit_type = value[6].try_into()?;
        let address_size = value[7];
        let debug_abbrev_offset = u32::from_le_bytes([value[8], value[9], value[10], value[11]]);

        let mut offset = 12;

        let dwo_id = match unit_type {
            UnitType::Skeleton | UnitType::SplitCompile => {
                let id = u64::from_le_bytes([
                    value[offset],
                    value[offset + 1],
                    value[offset + 2],
                    value[offset + 3],
                    value[offset + 4],
                    value[offset + 5],
                    value[offset + 6],
                    value[offset + 7],
                ]);
                offset += 8;
                Some(id)
            }
            _ => None,
        };

        let type_sig = match unit_type {
            UnitType::Type => {
                let sig = u64::from_le_bytes([
                    value[offset],
                    value[offset + 1],
                    value[offset + 2],
                    value[offset + 3],
                    value[offset + 4],
                    value[offset + 5],
                    value[offset + 6],
                    value[offset + 7],
                ]);
                offset += 8;
                Some(sig)
            }
            _ => None,
        };

        let type_offset = match unit_type {
            UnitType::Type => {
                let ty_of = u64::from_le_bytes([
                    value[offset],
                    value[offset + 1],
                    value[offset + 2],
                    value[offset + 3],
                    value[offset + 4],
                    value[offset + 5],
                    value[offset + 6],
                    value[offset + 7],
                ]);
                offset += 8;
                Some(ty_of)
            }
            _ => None,
        };

        if offset > total_unit_size {
            return Err("Calculated data offset exceeds total unit size".into());
        }
        let data = value[offset..total_unit_size].to_vec();

        Ok(Self {
            unit_length,
            version,
            unit_type,
            address_size,
            debug_abbrev_offset,
            dwo_id,
            type_sig,
            type_offset,
            data,
        })
    }
}

impl DebugInfo {
    fn size(&self) -> usize {
        4 + self.unit_length as usize
    }
}

// 7.5.3 Abbreviations Tables
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbbrevTag {
    ArrayType,
    ClassType,
    EntryPoint,
    EnumerationType,
    FormalParameter,
    ImportedDeclaration,
    Label,
    LexicalBlock,
    Member,
    PointerType,
    ReferenceType,
    CompileUnit,
    StringType,
    StructureType,
    SubroutineType,
    Typedef,
    UnionType,
    UnspecifiedParameters,
    Variant,
    CommonBlock,
    CommonInclusion,
    Inheritance,
    InlinedSubroutine,
    Module,
    PtrToMemberType,
    SetType,
    SubrangeType,
    WithStmt,
    AccessDeclaration,
    BaseType,
    CatchBlock,
    ConstType,
    Constant,
    Enumerator,
    FileType,
    Friend,
    Namelist,
    NamelistItem,
    PackedType,
    Subprogram,
    TemplateTypeParameter,
    TemplateValueParameter,
    ThrownType,
    TryBlock,
    VariantPart,
    Variable,
    VolatileType,
    DwarfProcedure,
    RestrictType,
    InterfaceType,
    Namespace,
    ImportedModule,
    UnspecifiedType,
    PartialUnit,
    ImportedUnit,
    Condition,
    SharedType,
    TypeUnit,
    RvalueReferenceType,
    TemplateAlias,
    CoarrayType,
    GenericSubrange,
    DynamicType,
    AtomicType,
    CallSite,
    CallSiteParameter,
    SkeletonUnit,
    ImmutableType,
    User(u64),
}

impl TryFrom<u64> for AbbrevTag {
    type Error = Error;

    fn try_from(value: u64) -> Result<Self> {
        match value {
            0x01 => Ok(Self::ArrayType),
            0x02 => Ok(Self::ClassType),
            0x03 => Ok(Self::EntryPoint),
            0x04 => Ok(Self::EnumerationType),
            0x05 => Ok(Self::FormalParameter),
            0x08 => Ok(Self::ImportedDeclaration),
            0x0a => Ok(Self::Label),
            0x0b => Ok(Self::LexicalBlock),
            0x0d => Ok(Self::Member),
            0x0f => Ok(Self::PointerType),
            0x10 => Ok(Self::ReferenceType),
            0x11 => Ok(Self::CompileUnit),
            0x12 => Ok(Self::StringType),
            0x13 => Ok(Self::StructureType),
            0x15 => Ok(Self::SubroutineType),
            0x16 => Ok(Self::Typedef),
            0x17 => Ok(Self::UnionType),
            0x18 => Ok(Self::UnspecifiedParameters),
            0x19 => Ok(Self::Variant),
            0x1a => Ok(Self::CommonBlock),
            0x1b => Ok(Self::CommonInclusion),
            0x1c => Ok(Self::Inheritance),
            0x1d => Ok(Self::InlinedSubroutine),
            0x1e => Ok(Self::Module),
            0x1f => Ok(Self::PtrToMemberType),
            0x20 => Ok(Self::SetType),
            0x21 => Ok(Self::SubrangeType),
            0x22 => Ok(Self::WithStmt),
            0x23 => Ok(Self::AccessDeclaration),
            0x24 => Ok(Self::BaseType),
            0x25 => Ok(Self::CatchBlock),
            0x26 => Ok(Self::ConstType),
            0x27 => Ok(Self::Constant),
            0x28 => Ok(Self::Enumerator),
            0x29 => Ok(Self::FileType),
            0x2a => Ok(Self::Friend),
            0x2b => Ok(Self::Namelist),
            0x2c => Ok(Self::NamelistItem),
            0x2d => Ok(Self::PackedType),
            0x2e => Ok(Self::Subprogram),
            0x2f => Ok(Self::TemplateTypeParameter),
            0x30 => Ok(Self::TemplateValueParameter),
            0x31 => Ok(Self::ThrownType),
            0x32 => Ok(Self::TryBlock),
            0x33 => Ok(Self::VariantPart),
            0x34 => Ok(Self::Variable),
            0x35 => Ok(Self::VolatileType),
            0x36 => Ok(Self::DwarfProcedure),
            0x37 => Ok(Self::RestrictType),
            0x38 => Ok(Self::InterfaceType),
            0x39 => Ok(Self::Namespace),
            0x3a => Ok(Self::ImportedModule),
            0x3b => Ok(Self::UnspecifiedType),
            0x3c => Ok(Self::PartialUnit),
            0x3d => Ok(Self::ImportedUnit),
            0x3f => Ok(Self::Condition),
            0x40 => Ok(Self::SharedType),
            0x41 => Ok(Self::TypeUnit),
            0x42 => Ok(Self::RvalueReferenceType),
            0x43 => Ok(Self::TemplateAlias),
            0x44 => Ok(Self::CoarrayType),
            0x45 => Ok(Self::GenericSubrange),
            0x46 => Ok(Self::DynamicType),
            0x47 => Ok(Self::AtomicType),
            0x48 => Ok(Self::CallSite),
            0x49 => Ok(Self::CallSiteParameter),
            0x4a => Ok(Self::SkeletonUnit),
            0x4b => Ok(Self::ImmutableType),
            0x4090..=0xffff => Ok(Self::User(value)),
            _ => Err("Invalid AbbrevTag value".into()),
        }
    }
}

// 7.5.4 Attribute Encodings
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbbrevAttribute {
    Sibling,
    Location,
    Name,
    Ordering,
    ByteSize,
    BitSize,
    StmtList,
    LowPc,
    HighPc,
    Language,
    Discr,
    DiscrValue,
    Visibility,
    Import,
    StringLength,
    CommonReference,
    CompDir,
    ConstValue,
    ContainingType,
    DefaultValue,
    Inline,
    IsOptional,
    LowerBound,
    Producer,
    Prototyped,
    ReturnAddr,
    StartScope,
    BitStride,
    UpperBound,
    AbstractOrigin,
    Accessibility,
    AddressClass,
    Artificial,
    BaseTypes,
    CallingConvention,
    Count,
    DataMemberLocation,
    DeclColumn,
    DeclFile,
    DeclLine,
    Declaration,
    DiscrList,
    Encoding,
    External,
    FrameBase,
    Friend,
    IdentifierCase,
    NameListItem,
    Priority,
    Segment,
    Specification,
    StaticLink,
    Type,
    UseLocation,
    VariableParameter,
    Virtuality,
    VtableElemLocation,
    Allocated,
    Associated,
    DataLocation,
    ByteStride,
    EntryPc,
    UseUtf8,
    Extension,
    Ranges,
    Trampoline,
    CallColumn,
    CallFile,
    CallLine,
    Description,
    BinaryScale,
    DecimalScale,
    Small,
    DecimalSign,
    DigitCount,
    PictureString,
    Mutable,
    ThreadsScaled,
    Explicit,
    ObjectPointer,
    Endianity,
    Elemental,
    Pure,
    Recursive,
    Signature,
    MainSubprogram,
    DataBitOffset,
    ConstExpr,
    EnumClass,
    LinkageName,
    StringLengthBitSize,
    StringLengthByteSize,
    Rank,
    StrOffsetsBase,
    AddrBase,
    RnglistsBase,
    DwoName,
    Reference,
    RvalueReference,
    Macros,
    CallAllCalls,
    CallAllSourceCalls,
    CallAllTailCalls,
    CallReturnPc,
    CallValue,
    CallOrigin,
    CallParameter,
    CallPc,
    CallTailCall,
    CallTarget,
    CallTargetClobbered,
    CallDataLocation,
    CallDataValue,
    Noreturn,
    Alignment,
    ExportSymbols,
    Deleted,
    Defaulted,
    LoclistsBase,
    User(u64),
}

impl TryFrom<u64> for AbbrevAttribute {
    type Error = Error;

    fn try_from(value: u64) -> Result<Self> {
        match value {
            0x01 => Ok(Self::Sibling),
            0x02 => Ok(Self::Location),
            0x03 => Ok(Self::Name),
            0x09 => Ok(Self::Ordering),
            0x0b => Ok(Self::ByteSize),
            0x0d => Ok(Self::BitSize),
            0x10 => Ok(Self::StmtList),
            0x11 => Ok(Self::LowPc),
            0x12 => Ok(Self::HighPc),
            0x13 => Ok(Self::Language),
            0x15 => Ok(Self::Discr),
            0x16 => Ok(Self::DiscrValue),
            0x17 => Ok(Self::Visibility),
            0x18 => Ok(Self::Import),
            0x19 => Ok(Self::StringLength),
            0x1a => Ok(Self::CommonReference),
            0x1b => Ok(Self::CompDir),
            0x1c => Ok(Self::ConstValue),
            0x1d => Ok(Self::ContainingType),
            0x1e => Ok(Self::DefaultValue),
            0x20 => Ok(Self::Inline),
            0x21 => Ok(Self::IsOptional),
            0x22 => Ok(Self::LowerBound),
            0x25 => Ok(Self::Producer),
            0x27 => Ok(Self::Prototyped),
            0x2a => Ok(Self::ReturnAddr),
            0x2c => Ok(Self::StartScope),
            0x2e => Ok(Self::BitStride),
            0x2f => Ok(Self::UpperBound),
            0x31 => Ok(Self::AbstractOrigin),
            0x32 => Ok(Self::Accessibility),
            0x33 => Ok(Self::AddressClass),
            0x34 => Ok(Self::Artificial),
            0x35 => Ok(Self::BaseTypes),
            0x36 => Ok(Self::CallingConvention),
            0x37 => Ok(Self::Count),
            0x38 => Ok(Self::DataMemberLocation),
            0x39 => Ok(Self::DeclColumn),
            0x3a => Ok(Self::DeclFile),
            0x3b => Ok(Self::DeclLine),
            0x3c => Ok(Self::Declaration),
            0x3d => Ok(Self::DiscrList),
            0x3e => Ok(Self::Encoding),
            0x3f => Ok(Self::External),
            0x40 => Ok(Self::FrameBase),
            0x41 => Ok(Self::Friend),
            0x42 => Ok(Self::IdentifierCase),
            0x44 => Ok(Self::NameListItem),
            0x45 => Ok(Self::Priority),
            0x46 => Ok(Self::Segment),
            0x47 => Ok(Self::Specification),
            0x48 => Ok(Self::StaticLink),
            0x49 => Ok(Self::Type),
            0x4a => Ok(Self::UseLocation),
            0x4b => Ok(Self::VariableParameter),
            0x4c => Ok(Self::Virtuality),
            0x4d => Ok(Self::VtableElemLocation),
            0x4e => Ok(Self::Allocated),
            0x4f => Ok(Self::Associated),
            0x50 => Ok(Self::DataLocation),
            0x51 => Ok(Self::ByteStride),
            0x52 => Ok(Self::EntryPc),
            0x53 => Ok(Self::UseUtf8),
            0x54 => Ok(Self::Extension),
            0x55 => Ok(Self::Ranges),
            0x56 => Ok(Self::Trampoline),
            0x57 => Ok(Self::CallColumn),
            0x58 => Ok(Self::CallFile),
            0x59 => Ok(Self::CallLine),
            0x5a => Ok(Self::Description),
            0x5b => Ok(Self::BinaryScale),
            0x5c => Ok(Self::DecimalScale),
            0x5d => Ok(Self::Small),
            0x5e => Ok(Self::DecimalSign),
            0x5f => Ok(Self::DigitCount),
            0x60 => Ok(Self::PictureString),
            0x61 => Ok(Self::Mutable),
            0x62 => Ok(Self::ThreadsScaled),
            0x63 => Ok(Self::Explicit),
            0x64 => Ok(Self::ObjectPointer),
            0x65 => Ok(Self::Endianity),
            0x66 => Ok(Self::Elemental),
            0x67 => Ok(Self::Pure),
            0x68 => Ok(Self::Recursive),
            0x69 => Ok(Self::Signature),
            0x6a => Ok(Self::MainSubprogram),
            0x6b => Ok(Self::DataBitOffset),
            0x6c => Ok(Self::ConstExpr),
            0x6d => Ok(Self::EnumClass),
            0x6e => Ok(Self::LinkageName),
            0x6f => Ok(Self::StringLengthBitSize),
            0x70 => Ok(Self::StringLengthByteSize),
            0x71 => Ok(Self::Rank),
            0x72 => Ok(Self::StrOffsetsBase),
            0x73 => Ok(Self::AddrBase),
            0x74 => Ok(Self::RnglistsBase),
            0x76 => Ok(Self::DwoName),
            0x77 => Ok(Self::Reference),
            0x78 => Ok(Self::RvalueReference),
            0x79 => Ok(Self::Macros),
            0x7a => Ok(Self::CallAllCalls),
            0x7b => Ok(Self::CallAllSourceCalls),
            0x7c => Ok(Self::CallAllTailCalls),
            0x7d => Ok(Self::CallReturnPc),
            0x7e => Ok(Self::CallValue),
            0x7f => Ok(Self::CallOrigin),
            0x80 => Ok(Self::CallParameter),
            0x81 => Ok(Self::CallPc),
            0x82 => Ok(Self::CallTailCall),
            0x83 => Ok(Self::CallTarget),
            0x84 => Ok(Self::CallTargetClobbered),
            0x85 => Ok(Self::CallDataLocation),
            0x86 => Ok(Self::CallDataValue),
            0x87 => Ok(Self::Noreturn),
            0x88 => Ok(Self::Alignment),
            0x89 => Ok(Self::ExportSymbols),
            0x8a => Ok(Self::Deleted),
            0x8b => Ok(Self::Defaulted),
            0x8c => Ok(Self::LoclistsBase),
            0x2000..=0x3fff => Ok(Self::User(value)),
            _ => Err("Invalid AbbrevAttribute value".into()),
        }
    }
}

// 7.5.6 Form Encodings
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbbrevForm {
    Addr(u64),
    Block2,
    Block4,
    Data2(u16),
    Data4(u32),
    Data8(u64),
    String(String),
    Block,
    Block1,
    Data1(u8),
    Flag(bool),
    Sdata(i64), // sleb128
    Strp(String),
    Udata(u64), // uleb128
    RefAddr,
    Ref1(usize), // offset of .debug_info
    Ref2(usize),
    Ref4(usize),
    Ref8(usize),
    RefUdata,
    Indirect(u64),
    SecOffset(Option<u32>),
    Exprloc(Vec<u8>),
    FlagPresent,
    Strx,
    Addrx,
    RefSup4,
    StrpSup(String),
    Data16(u128),
    LineStrp(String),
    RefSig8,
    ImplicitConst(i64),
    Loclistx,
    Rnglistx,
    RefSup8,
    Strx1,
    Strx2,
    Strx3,
    Strx4,
    Addrx1,
    Addrx2,
    Addrx3,
    Addrx4,
}

impl TryFrom<u64> for AbbrevForm {
    type Error = Error;

    fn try_from(value: u64) -> Result<Self> {
        match value {
            0x01 => Ok(Self::Addr(0)),
            0x03 => Ok(Self::Block2),
            0x04 => Ok(Self::Block4),
            0x05 => Ok(Self::Data2(0)),
            0x06 => Ok(Self::Data4(0)),
            0x07 => Ok(Self::Data8(0)),
            0x08 => Ok(Self::String(String::new())),
            0x09 => Ok(Self::Block),
            0x0a => Ok(Self::Block1),
            0x0b => Ok(Self::Data1(0)),
            0x0c => Ok(Self::Flag(false)),
            0x0d => Ok(Self::Sdata(0)),
            0x0e => Ok(Self::Strp(String::new())),
            0x0f => Ok(Self::Udata(0)),
            0x10 => Ok(Self::RefAddr),
            0x11 => Ok(Self::Ref1(0)),
            0x12 => Ok(Self::Ref2(0)),
            0x13 => Ok(Self::Ref4(0)),
            0x14 => Ok(Self::Ref8(0)),
            0x15 => Ok(Self::RefUdata),
            0x16 => Ok(Self::Indirect(0)),
            0x17 => Ok(Self::SecOffset(None)),
            0x18 => Ok(Self::Exprloc(Vec::new())),
            0x19 => Ok(Self::FlagPresent),
            0x1a => Ok(Self::Strx),
            0x1b => Ok(Self::Addrx),
            0x1c => Ok(Self::RefSup4),
            0x1d => Ok(Self::StrpSup(String::new())),
            0x1e => Ok(Self::Data16(0)),
            0x1f => Ok(Self::LineStrp(String::new())),
            0x20 => Ok(Self::RefSig8),
            0x21 => Ok(Self::ImplicitConst(0)),
            0x22 => Ok(Self::Loclistx),
            0x23 => Ok(Self::Rnglistx),
            0x24 => Ok(Self::RefSup8),
            0x25 => Ok(Self::Strx1),
            0x26 => Ok(Self::Strx2),
            0x27 => Ok(Self::Strx3),
            0x28 => Ok(Self::Strx4),
            0x29 => Ok(Self::Addrx1),
            0x2a => Ok(Self::Addrx2),
            0x2b => Ok(Self::Addrx3),
            0x2c => Ok(Self::Addrx4),
            _ => Err("Invalid AbbrevForm value".into()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DebugAbbrev {
    pub code: u64,
    pub tag: AbbrevTag,
    pub has_children: bool,
    pub attributes: Vec<(AbbrevAttribute, AbbrevForm)>,
}

impl DebugAbbrev {
    pub fn is_contains_by_ip(&self, ip: u64) -> bool {
        let mut low_pc = None;
        let mut high_pc = None;

        for (attr, form) in &self.attributes {
            match (attr, form) {
                (AbbrevAttribute::LowPc, AbbrevForm::Addr(addr)) => {
                    low_pc = Some(*addr);
                }
                (AbbrevAttribute::HighPc, AbbrevForm::Data8(data)) => {
                    high_pc = Some(*data);
                }
                _ => (),
            }
        }

        if let (Some(low), Some(high)) = (low_pc, high_pc) {
            return low <= ip && ip < (low + high);
        }

        false
    }
}

// Table 7.27: Line number header entry format encodings
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineNumberHeaderEntry {
    Path,
    DirectoryIndex,
    Timestamp,
    Size,
    Md5,
    User(u64),
}

impl TryFrom<u64> for LineNumberHeaderEntry {
    type Error = Error;

    fn try_from(value: u64) -> Result<Self> {
        match value {
            0x01 => Ok(Self::Path),
            0x02 => Ok(Self::DirectoryIndex),
            0x03 => Ok(Self::Timestamp),
            0x04 => Ok(Self::Size),
            0x05 => Ok(Self::Md5),
            0x06..=0xffff => Ok(Self::User(value)),
            _ => Err("Invalid LineNumberHeaderEntry value".into()),
        }
    }
}

// 6.2.4 The Line Number Program Header
#[derive(Clone)]
pub struct DebugLine {
    pub unit_length: u32,
    pub version: u16,
    pub address_size: u8,
    pub segment_selector_size: u8,
    pub header_length: u32,
    pub minimum_instruction_length: u8,
    pub maximum_operations_per_instruction: u8,
    pub default_is_stmt: bool,
    pub line_base: i8,
    pub line_range: u8,
    pub opcode_base: u8,
    pub standard_opcode_lengths: Vec<u8>,
    pub directory_entry_format_count: u8,
    pub directory_entry_format: Vec<(LineNumberHeaderEntry, AbbrevForm)>,
    pub directories_count: u64,
    pub directories: Vec<String>,
    pub file_name_entry_format_count: u8,
    pub file_name_entry_format: Vec<(LineNumberHeaderEntry, AbbrevForm)>,
    pub file_names_count: u64,
    pub file_names: Vec<String>,
    program: Vec<u8>,
}

impl core::fmt::Debug for DebugLine {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DebugLine")
            .field("unit_length", &self.unit_length)
            .field("version", &self.version)
            .field("address_size", &self.address_size)
            .field("segment_selector_size", &self.segment_selector_size)
            .field("header_length", &self.header_length)
            .field(
                "minimum_instruction_length",
                &self.minimum_instruction_length,
            )
            .field(
                "maximum_operations_per_instruction",
                &self.maximum_operations_per_instruction,
            )
            .field("default_is_stmt", &self.default_is_stmt)
            .field("line_base", &self.line_base)
            .field("line_range", &self.line_range)
            .field("opcode_base", &self.opcode_base)
            .field("standard_opcode_lengths", &self.standard_opcode_lengths)
            .field(
                "directory_entry_format_count",
                &self.directory_entry_format_count,
            )
            .field("directory_entry_format", &self.directory_entry_format)
            .field("directories_count", &self.directories_count)
            .field("directories", &self.directories)
            .field(
                "file_name_entry_format_count",
                &self.file_name_entry_format_count,
            )
            .field("file_name_entry_format", &self.file_name_entry_format)
            .field("file_names_count", &self.file_names_count)
            .field("file_names", &self.file_names)
            .finish()
    }
}

impl DebugLine {
    pub fn try_from(value: &[u8], debug_line_str_slice: &[u8]) -> Result<Self> {
        if value.len() < 4 {
            return Err("Invalid DebugLine length (unit_length)".into());
        }

        let unit_length = u32::from_le_bytes([value[0], value[1], value[2], value[3]]);

        if unit_length == 0xffff_ffff {
            return Err("64-bit DWARF format is not supported".into());
        }

        let total_unit_size = 4 + unit_length as usize;
        if value.len() < total_unit_size {
            return Err("DebugLine data section out of bounds (unit_length mismatch)".into());
        }

        let version = u16::from_le_bytes([value[4], value[5]]);
        if version != 5 {
            return Err("Unsupported DWARF version".into());
        }

        let address_size = value[6];
        let segment_selector_size = value[7];
        let header_length = u32::from_le_bytes([value[8], value[9], value[10], value[11]]);
        let minimum_instruction_length = value[12];
        let maximum_operations_per_instruction = value[13];
        let default_is_stmt = value[14] != 0;
        let line_base = value[15] as i8;
        let line_range = value[16];
        let opcode_base = value[17];

        let mut offset = 18;
        let mut standard_opcode_lengths = Vec::new();
        for _ in 0..opcode_base - 1 {
            standard_opcode_lengths.push(value[offset]);
            offset += 1;
        }

        let directory_entry_format_count = value[offset];
        offset += 1;

        let mut directory_entry_format = Vec::new();
        for _ in 0..directory_entry_format_count {
            let entry = read_uleb128(&value, &mut offset);
            let format = read_uleb128(&value, &mut offset);
            directory_entry_format.push((entry.try_into()?, format.try_into()?));
        }

        let directories_count = read_uleb128(&value, &mut offset);

        let mut directories = Vec::new();
        for _ in 0..directories_count {
            for (entry, form) in &directory_entry_format {
                match (entry, form) {
                    (LineNumberHeaderEntry::Path, AbbrevForm::LineStrp(_)) => {
                        let s_offset = u32::from_le_bytes([
                            value[offset],
                            value[offset + 1],
                            value[offset + 2],
                            value[offset + 3],
                        ]);
                        offset += 4;
                        let s =
                            util::cstring::from_slice(&debug_line_str_slice[s_offset as usize..]);
                        directories.push(s);
                    }
                    _ => unimplemented!(),
                }
            }
        }

        let file_name_entry_format_count = value[offset];
        offset += 1;

        let mut file_name_entry_format = Vec::new();
        for _ in 0..file_name_entry_format_count {
            let entry = read_uleb128(&value, &mut offset);
            let format = read_uleb128(&value, &mut offset);
            file_name_entry_format.push((entry.try_into()?, format.try_into()?));
        }

        let file_names_count = read_uleb128(&value, &mut offset);

        let mut file_names = Vec::new();
        for (entry, form) in &file_name_entry_format {
            match (entry, form) {
                (LineNumberHeaderEntry::Path, AbbrevForm::LineStrp(_)) => {
                    let s_offset = u32::from_le_bytes([
                        value[offset],
                        value[offset + 1],
                        value[offset + 2],
                        value[offset + 3],
                    ]);
                    offset += 4;
                    let s = util::cstring::from_slice(&debug_line_str_slice[s_offset as usize..]);
                    file_names.push(s);
                }
                (LineNumberHeaderEntry::DirectoryIndex, AbbrevForm::Udata(_)) => {
                    let s_offset = read_uleb128(&value, &mut offset);
                    let s = util::cstring::from_slice(&debug_line_str_slice[s_offset as usize..]);
                    file_names.push(s);
                }
                _ => unimplemented!(),
            }
        }

        let program = value[offset..total_unit_size].to_vec();

        Ok(Self {
            unit_length,
            version,
            address_size,
            segment_selector_size,
            header_length,
            minimum_instruction_length,
            maximum_operations_per_instruction,
            default_is_stmt,
            line_base,
            line_range,
            opcode_base,
            standard_opcode_lengths,
            directory_entry_format_count,
            directory_entry_format,
            directories_count,
            directories,
            file_name_entry_format_count,
            file_name_entry_format,
            file_names_count,
            file_names,
            program,
        })
    }
}

fn read_uleb128(slice: &[u8], offset: &mut usize) -> u64 {
    let mut res = 0;
    let mut shift = 0;

    while *offset < slice.len() {
        let byte = slice[*offset];
        *offset += 1;
        res |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
    }

    res
}

fn read_leb128(slice: &[u8], offset: &mut usize) -> i64 {
    let mut res = 0;
    let mut shift = 0;
    let mut byte = 0;

    while *offset < slice.len() {
        byte = slice[*offset];
        *offset += 1;
        res |= ((byte & 0x7f) as i64) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
    }

    // sign extend
    if (shift < 64) && (byte & 0x40 != 0) {
        res |= !0 << shift;
    }

    res
}

fn parse_debug_abbrev(
    debug_abbrev_slice: &[u8],
    offset: usize,
) -> Result<BTreeMap<u64, DebugAbbrev>> {
    let mut offset = offset;
    let mut debug_abbrevs = BTreeMap::new();

    while offset < debug_abbrev_slice.len() {
        let code = read_uleb128(debug_abbrev_slice, &mut offset);
        if code == 0 {
            break; // null entry
        }

        let tag = read_uleb128(debug_abbrev_slice, &mut offset).try_into()?;
        let has_children = match debug_abbrev_slice[offset] {
            0 => false,
            1 => true,
            _ => return Err("Invalid has_children value".into()),
        };
        offset += 1;

        let mut attributes = Vec::new();
        loop {
            let name = read_uleb128(debug_abbrev_slice, &mut offset);
            let form = read_uleb128(debug_abbrev_slice, &mut offset);

            if name == 0 && form == 0 {
                break; // null entry
            }

            let attr = name.try_into()?;
            let mut form = form.try_into()?;

            match form {
                AbbrevForm::Indirect(ref mut v) => {
                    *v = read_uleb128(debug_abbrev_slice, &mut offset);
                }
                AbbrevForm::ImplicitConst(ref mut v) => {
                    *v = read_leb128(debug_abbrev_slice, &mut offset);
                }
                _ => (),
            }

            attributes.push((attr, form));
        }

        debug_abbrevs.insert(
            code,
            DebugAbbrev {
                code,
                tag,
                has_children,
                attributes,
            },
        );
    }

    Ok(debug_abbrevs)
}

fn parse_debug_info(debug_info_slice: &[u8]) -> Result<Vec<DebugInfo>> {
    let mut debug_infos = Vec::new();
    let mut offset = 0;

    while offset < debug_info_slice.len() {
        let debug_info = DebugInfo::try_from(&debug_info_slice[offset..])?;
        offset += debug_info.size();
        debug_infos.push(debug_info);
    }

    Ok(debug_infos)
}

fn parse_die(
    debug_abbrev_slice: &[u8],
    debug_str_slice: &[u8],
    debug_line_str_slice: &[u8],
    debug_addr_slice: Option<&[u8]>,
    debug_info: &DebugInfo,
) -> Result<BTreeMap<u64, DebugAbbrev>> {
    let debug_abbrev_offset = debug_info.debug_abbrev_offset as usize;
    let mut debug_abbrevs = parse_debug_abbrev(debug_abbrev_slice, debug_abbrev_offset)?;

    let die_data: &[u8] = &debug_info.data;
    let mut offset = 0;
    while offset < die_data.len() {
        let code = read_uleb128(die_data, &mut offset);
        if code == 0 {
            continue;
        }

        let abbrev = debug_abbrevs
            .get_mut(&code)
            .ok_or::<Error>("Failed to find abbrev".into())?;

        for (_, form) in &mut abbrev.attributes {
            match form {
                AbbrevForm::Addr(ref mut v) => {
                    match debug_info.address_size {
                        4 => {
                            *v = u32::from_le_bytes([
                                die_data[offset],
                                die_data[offset + 1],
                                die_data[offset + 2],
                                die_data[offset + 3],
                            ]) as u64;
                        }
                        8 => {
                            *v = u64::from_le_bytes([
                                die_data[offset],
                                die_data[offset + 1],
                                die_data[offset + 2],
                                die_data[offset + 3],
                                die_data[offset + 4],
                                die_data[offset + 5],
                                die_data[offset + 6],
                                die_data[offset + 7],
                            ]);
                        }
                        _ => unreachable!(),
                    }

                    offset += debug_info.address_size as usize;
                }
                AbbrevForm::SecOffset(ref mut v) => {
                    if let Some(debug_addr_slice) = debug_addr_slice {
                        let addr_offset = u32::from_le_bytes([
                            die_data[offset],
                            die_data[offset + 1],
                            die_data[offset + 2],
                            die_data[offset + 3],
                        ]) as usize;
                        *v = Some(u32::from_le_bytes([
                            debug_addr_slice[addr_offset],
                            debug_addr_slice[addr_offset + 1],
                            debug_addr_slice[addr_offset + 2],
                            debug_addr_slice[addr_offset + 3],
                        ]));
                    } else {
                        *v = None;
                    }
                    offset += 4;
                }
                AbbrevForm::Strp(ref mut s) => {
                    let str_offset = u32::from_le_bytes([
                        die_data[offset],
                        die_data[offset + 1],
                        die_data[offset + 2],
                        die_data[offset + 3],
                    ]) as usize;
                    offset += 4;
                    *s = util::cstring::from_slice(&debug_str_slice[str_offset..]);
                }
                AbbrevForm::LineStrp(ref mut s) => {
                    let str_offset = u32::from_le_bytes([
                        die_data[offset],
                        die_data[offset + 1],
                        die_data[offset + 2],
                        die_data[offset + 3],
                    ]) as usize;
                    offset += 4;
                    *s = util::cstring::from_slice(&debug_line_str_slice[str_offset..]);
                }
                AbbrevForm::StrpSup(ref mut s) => {
                    let str_offset = u32::from_le_bytes([
                        die_data[offset],
                        die_data[offset + 1],
                        die_data[offset + 2],
                        die_data[offset + 3],
                    ]) as usize;
                    offset += 4;
                    *s = util::cstring::from_slice(&debug_str_slice[str_offset..]);
                }
                AbbrevForm::String(ref mut s) => {
                    let cs = util::cstring::from_slice(&die_data[offset..]);
                    offset += cs.len() + 1;
                    *s = cs;
                }
                AbbrevForm::Exprloc(ref mut v) => {
                    let expr_len = read_uleb128(die_data, &mut offset) as usize;
                    *v = die_data[offset..offset + expr_len].to_vec();
                    offset += expr_len;
                }
                AbbrevForm::Flag(ref mut v) => {
                    *v = die_data[offset] != 0;
                    offset += 1;
                }
                AbbrevForm::Data1(ref mut v) => {
                    *v = die_data[offset];
                    offset += 1;
                }
                AbbrevForm::Data2(ref mut v) => {
                    *v = u16::from_le_bytes([die_data[offset], die_data[offset + 1]]);
                    offset += 2;
                }
                AbbrevForm::Data4(ref mut v) => {
                    *v = u32::from_le_bytes([
                        die_data[offset],
                        die_data[offset + 1],
                        die_data[offset + 2],
                        die_data[offset + 3],
                    ]);
                    offset += 4;
                }
                AbbrevForm::Data8(ref mut v) => {
                    *v = u64::from_le_bytes([
                        die_data[offset],
                        die_data[offset + 1],
                        die_data[offset + 2],
                        die_data[offset + 3],
                        die_data[offset + 4],
                        die_data[offset + 5],
                        die_data[offset + 6],
                        die_data[offset + 7],
                    ]);
                    offset += 8;
                }
                AbbrevForm::Data16(ref mut v) => {
                    *v = u128::from_le_bytes([
                        die_data[offset],
                        die_data[offset + 1],
                        die_data[offset + 2],
                        die_data[offset + 3],
                        die_data[offset + 4],
                        die_data[offset + 5],
                        die_data[offset + 6],
                        die_data[offset + 7],
                        die_data[offset + 8],
                        die_data[offset + 9],
                        die_data[offset + 10],
                        die_data[offset + 11],
                        die_data[offset + 12],
                        die_data[offset + 13],
                        die_data[offset + 14],
                        die_data[offset + 15],
                    ]);
                    offset += 16;
                }
                AbbrevForm::Ref1(ref mut v) => {
                    *v = die_data[offset] as usize;
                    offset += 1;
                }
                AbbrevForm::Ref2(ref mut v) => {
                    *v = u16::from_le_bytes([die_data[offset], die_data[offset + 1]]) as usize;
                    offset += 2;
                }
                AbbrevForm::Ref4(ref mut v) => {
                    *v = u32::from_le_bytes([
                        die_data[offset],
                        die_data[offset + 1],
                        die_data[offset + 2],
                        die_data[offset + 3],
                    ]) as usize;
                    offset += 4;
                }
                AbbrevForm::Ref8(ref mut v) => {
                    *v = u64::from_le_bytes([
                        die_data[offset],
                        die_data[offset + 1],
                        die_data[offset + 2],
                        die_data[offset + 3],
                        die_data[offset + 4],
                        die_data[offset + 5],
                        die_data[offset + 6],
                        die_data[offset + 7],
                    ]) as usize;
                    offset += 8;
                }
                AbbrevForm::Indirect(_)
                | AbbrevForm::ImplicitConst(_)
                | AbbrevForm::FlagPresent => {
                    // skip
                }
                _ => {
                    unimplemented!("{:?}", form)
                }
            }
        }
    }

    Ok(debug_abbrevs)
}

fn parse_debug_line(
    debug_line_slice: &[u8],
    debug_line_str_slice: &[u8],
) -> Result<Vec<DebugLine>> {
    let mut debug_lines = Vec::new();
    let mut offset = 0;

    while offset < debug_line_slice.len() {
        let debug_line = DebugLine::try_from(&debug_line_slice[offset..], debug_line_str_slice)?;
        offset += debug_line.unit_length as usize + 4; // 4 bytes for unit_length
        debug_lines.push(debug_line);
    }

    Ok(debug_lines)
}

#[derive(Debug, Clone)]
pub struct Dwarf {
    pub die_tree: Vec<(DebugInfo, BTreeMap<u64, DebugAbbrev>)>,
    pub debug_lines: Vec<DebugLine>,
}

impl Dwarf {
    pub fn find_debug_info_by_ip(&self, ip: u64) -> Option<Vec<(&DebugInfo, Vec<&DebugAbbrev>)>> {
        let mut result = Vec::new();

        for (debug_info, debug_abbrevs) in &self.die_tree {
            let mut abbrevs = Vec::new();
            for abbrev in debug_abbrevs.values() {
                if abbrev.is_contains_by_ip(ip) {
                    abbrevs.extend(debug_abbrevs.values());
                    break;
                }
            }

            if !abbrevs.is_empty() {
                result.push((debug_info, abbrevs));
            }
        }

        if !result.is_empty() {
            return Some(result);
        }

        None
    }
}

pub fn parse(elf64: &Elf64) -> Result<Dwarf> {
    let debug_info_sh = elf64
        .section_header_by_name(".debug_info")
        .ok_or::<Error>("Failed to find .debug_info section".into())?;

    let debug_info_slice = elf64
        .data_by_section_header(debug_info_sh)
        .ok_or::<Error>("Failed to get .debug_info section data".into())?;

    let debug_abbrev_sh = elf64
        .section_header_by_name(".debug_abbrev")
        .ok_or::<Error>("Failed to find .debug_abbrev section".into())?;

    let debug_abbrev_slice = elf64
        .data_by_section_header(debug_abbrev_sh)
        .ok_or::<Error>("Failed to get .debug_abbrev section data".into())?;

    let debug_str_sh = elf64
        .section_header_by_name(".debug_str")
        .ok_or::<Error>("Failed to find .debug_str section".into())?;

    let debug_str_slice = elf64
        .data_by_section_header(debug_str_sh)
        .ok_or::<Error>("Failed to get .debug_str section data".into())?;

    let debug_line_str_sh = elf64
        .section_header_by_name(".debug_line_str")
        .ok_or::<Error>("Failed to find .debug_line_str section".into())?;

    let debug_line_str_slice = elf64
        .data_by_section_header(debug_line_str_sh)
        .ok_or::<Error>("Failed to get .debug_line_str section data".into())?;

    let debug_addr_sh = elf64.section_header_by_name(".debug_addr");
    let debug_addr_slice = if let Some(debug_addr_sh) = debug_addr_sh {
        let slice = elf64
            .data_by_section_header(debug_addr_sh)
            .ok_or::<Error>("Failed to get .debug_addr section data".into())?;
        Some(slice)
    } else {
        None
    };

    // parse DIE syntax tree
    let mut die_tree = Vec::new();

    let debug_infos = parse_debug_info(debug_info_slice)?;
    for debug_info in &debug_infos {
        let debug_abbrebs = parse_die(
            debug_abbrev_slice,
            debug_str_slice,
            debug_line_str_slice,
            debug_addr_slice,
            debug_info,
        )?;

        die_tree.push((debug_info.clone(), debug_abbrebs));
    }

    // parse debug line
    let debug_line_sh = elf64
        .section_header_by_name(".debug_line")
        .ok_or::<Error>("Failed to find .debug_line section".into())?;
    let debug_line_slice = elf64
        .data_by_section_header(debug_line_sh)
        .ok_or::<Error>("Failed to get .debug_line section data".into())?;

    let debug_lines = parse_debug_line(debug_line_slice, debug_line_str_slice)?;

    Ok(Dwarf {
        die_tree,
        debug_lines,
    })
}
