use prost::Message;



pub type SendPropertyFlags = i32;
pub trait SendPropertyFlagsTrait {
    fn has_flag_set(self, flag: SendPropertyFlags) -> bool;
}
impl SendPropertyFlagsTrait for SendPropertyFlags {
    #[inline]
    fn has_flag_set(self, flag: SendPropertyFlags) -> bool {
        self&flag == flag
    }
}

#[derive(Debug, Clone)]
pub struct SendTable {
    pub properties: Vec<SendTableProperty>,
    pub name: String,
    pub is_end: bool,
    pub index: i32,
}

impl From<CsvcMsgSendTable> for SendTable {
    fn from(st: CsvcMsgSendTable) -> Self {
        let mut properties = Vec::with_capacity(st.props.len());
        properties.extend(st.props.iter().map(SendTableProperty::from));
        Self {
            properties,
            name: st.net_table_name().to_string(),
            is_end: st.is_end(),
            index: -1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SendTableProperty {
    pub flags: SendPropertyFlags,
    pub name: String,
    pub dt_name: String,
    pub low_value: f32,
    pub high_value: f32,
    pub num_bits: i32,
    pub num_elems: i32,
    pub priority: i32,
    pub raw_type: i32,
}

impl From<&SendpropT> for SendTableProperty {
    fn from(prop: &SendpropT) -> Self {
        Self {
            flags: prop.flags,
            dt_name: prop.dt_name().to_owned(),
            high_value: prop.high_value(),
            low_value: prop.low_value(),
            name: prop.var_name.to_owned(),
            num_bits: prop.num_bits,
            num_elems: prop.num_elements(),
            priority: prop.priority,
            raw_type: prop.r#type,
        }
    }
}


#[derive(Debug, Clone)]
pub struct ExcludeEntry {
    pub var_name: String,
    pub dt_name: String,
    pub excluding_dt: String,
}


#[derive(Clone, Message)]
pub struct CsvcMsgSendTable {
    #[prost(bool, optional, tag="1")]
    pub is_end: Option<bool>,
    #[prost(string, optional, tag="2")]
    pub net_table_name: Option<String>,
    #[prost(bool, optional, tag="3")]
    pub needs_decoder: Option<bool>,
    #[prost(message, repeated, tag="4")]
    pub props: Vec<SendpropT>,
}


#[derive(Clone, Message)]
pub struct SendpropT {
    #[prost(int32, tag="1")]
    pub r#type: i32,
    #[prost(string, tag="2")]
    pub var_name: String,
    #[prost(int32, tag="3")]
    pub flags: i32,
    #[prost(int32, tag="4")]
    pub priority: i32,
    #[prost(string, optional, tag="5")]
    pub dt_name: Option<String>,
    #[prost(int32, optional, tag="6")]
    pub num_elements: Option<i32>,
    #[prost(float, optional, tag="7")]
    pub low_value: Option<f32>,
    #[prost(float, optional, tag="8")]
    pub high_value: Option<f32>,
    #[prost(int32, tag="9")]
    pub num_bits: i32,
}