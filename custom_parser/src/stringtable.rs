use prost::Message;



#[derive(Clone, PartialEq, Eq, Message)]
pub struct CsvcMsgCreateStringTable {
    #[prost(string, optional, tag="1")]
    pub name: Option<String>,
    #[prost(int32, optional, tag="2")]
    pub max_entries: Option<i32>,
    #[prost(int32, optional, tag="3")]
    pub num_entries: Option<i32>,
    #[prost(bool, optional, tag="4")]
    pub user_data_fixed_size: Option<bool>,
    #[prost(int32, optional, tag="5")]
    pub user_data_size: Option<i32>,
    #[prost(int32, optional, tag="6")]
    pub user_data_size_bits: Option<i32>,
    #[prost(int32, optional, tag="7")]
    pub flags: Option<i32>,
    #[prost(bytes="vec", optional, tag="8")]
    pub string_data: Option<Vec<u8>>,
}

#[derive(Clone, Message)]
pub struct CsvcMsgUpdateStringTable {
    #[prost(int32, optional, tag="1")]
    pub table_id: Option<i32>,
    #[prost(int32, optional, tag="2")]
    pub num_changed_entries: Option<i32>,
    #[prost(bytes="vec", optional, tag="3")]
    pub string_data: Option<Vec<u8>>,
}