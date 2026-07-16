#![allow(clippy::redundant_closure_call)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::match_single_binding)]
#![allow(clippy::clone_on_copy)]

#[doc = r" Error types."]
pub mod error {
    #[doc = r" Error from a `TryFrom` or `FromStr` implementation."]
    pub struct ConversionError(::std::borrow::Cow<'static, str>);
    impl ::std::error::Error for ConversionError {}
    impl ::std::fmt::Display for ConversionError {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> Result<(), ::std::fmt::Error> {
            ::std::fmt::Display::fmt(&self.0, f)
        }
    }
    impl ::std::fmt::Debug for ConversionError {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> Result<(), ::std::fmt::Error> {
            ::std::fmt::Debug::fmt(&self.0, f)
        }
    }
    impl From<&'static str> for ConversionError {
        fn from(value: &'static str) -> Self {
            Self(value.into())
        }
    }
    impl From<String> for ConversionError {
        fn from(value: String) -> Self {
            Self(value.into())
        }
    }
}
#[doc = "`GeneratorQualification`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"$id\": \"https://schemas.sapphirus.dev/codegen/generator-qualification.schema.json\","]
#[doc = "  \"title\": \"GeneratorQualification\","]
#[doc = "  \"oneOf\": ["]
#[doc = "    {"]
#[doc = "      \"title\": \"GeneratorQualificationWithoutOptionalValue\","]
#[doc = "      \"type\": \"object\","]
#[doc = "      \"required\": ["]
#[doc = "        \"externalValue\","]
#[doc = "        \"interoperableValue\","]
#[doc = "        \"items\","]
#[doc = "        \"label\","]
#[doc = "        \"node\","]
#[doc = "        \"nullableValue\","]
#[doc = "        \"schemaVersion\","]
#[doc = "        \"signedValue\","]
#[doc = "        \"unsignedValue\","]
#[doc = "        \"variant\""]
#[doc = "      ],"]
#[doc = "      \"properties\": {"]
#[doc = "        \"externalValue\": {"]
#[doc = "          \"$ref\": \"#/$defs/QualificationExternalExternalValue\""]
#[doc = "        },"]
#[doc = "        \"interoperableValue\": {"]
#[doc = "          \"type\": \"integer\","]
#[doc = "          \"maximum\": 9007199254740991.0,"]
#[doc = "          \"minimum\": -9007199254740991.0"]
#[doc = "        },"]
#[doc = "        \"items\": {"]
#[doc = "          \"type\": \"array\","]
#[doc = "          \"items\": {"]
#[doc = "            \"type\": \"string\""]
#[doc = "          },"]
#[doc = "          \"minItems\": 0"]
#[doc = "        },"]
#[doc = "        \"label\": {"]
#[doc = "          \"type\": \"string\""]
#[doc = "        },"]
#[doc = "        \"node\": {"]
#[doc = "          \"$ref\": \"#/$defs/GeneratorQualificationNode\""]
#[doc = "        },"]
#[doc = "        \"nullableValue\": {"]
#[doc = "          \"type\": ["]
#[doc = "            \"string\","]
#[doc = "            \"null\""]
#[doc = "          ]"]
#[doc = "        },"]
#[doc = "        \"schemaVersion\": {"]
#[doc = "          \"const\": \"sapphirus.generator-qualification.v1\""]
#[doc = "        },"]
#[doc = "        \"signedValue\": {"]
#[doc = "          \"type\": \"integer\","]
#[doc = "          \"maximum\": 2147483647.0,"]
#[doc = "          \"minimum\": -2147483648.0"]
#[doc = "        },"]
#[doc = "        \"unsignedValue\": {"]
#[doc = "          \"type\": \"integer\","]
#[doc = "          \"maximum\": 4294967295.0,"]
#[doc = "          \"minimum\": 0.0"]
#[doc = "        },"]
#[doc = "        \"variant\": {"]
#[doc = "          \"oneOf\": ["]
#[doc = "            {"]
#[doc = "              \"$ref\": \"#/$defs/GeneratorQualificationTextVariant\""]
#[doc = "            },"]
#[doc = "            {"]
#[doc = "              \"$ref\": \"#/$defs/GeneratorQualificationCountVariant\""]
#[doc = "            }"]
#[doc = "          ]"]
#[doc = "        }"]
#[doc = "      },"]
#[doc = "      \"additionalProperties\": false"]
#[doc = "    },"]
#[doc = "    {"]
#[doc = "      \"title\": \"GeneratorQualificationWithOptionalValue\","]
#[doc = "      \"type\": \"object\","]
#[doc = "      \"required\": ["]
#[doc = "        \"externalValue\","]
#[doc = "        \"interoperableValue\","]
#[doc = "        \"items\","]
#[doc = "        \"label\","]
#[doc = "        \"node\","]
#[doc = "        \"nullableValue\","]
#[doc = "        \"optionalValue\","]
#[doc = "        \"schemaVersion\","]
#[doc = "        \"signedValue\","]
#[doc = "        \"unsignedValue\","]
#[doc = "        \"variant\""]
#[doc = "      ],"]
#[doc = "      \"properties\": {"]
#[doc = "        \"externalValue\": {"]
#[doc = "          \"$ref\": \"#/$defs/QualificationExternalExternalValue\""]
#[doc = "        },"]
#[doc = "        \"interoperableValue\": {"]
#[doc = "          \"type\": \"integer\","]
#[doc = "          \"maximum\": 9007199254740991.0,"]
#[doc = "          \"minimum\": -9007199254740991.0"]
#[doc = "        },"]
#[doc = "        \"items\": {"]
#[doc = "          \"type\": \"array\","]
#[doc = "          \"items\": {"]
#[doc = "            \"type\": \"string\""]
#[doc = "          },"]
#[doc = "          \"minItems\": 0"]
#[doc = "        },"]
#[doc = "        \"label\": {"]
#[doc = "          \"type\": \"string\""]
#[doc = "        },"]
#[doc = "        \"node\": {"]
#[doc = "          \"$ref\": \"#/$defs/GeneratorQualificationNode\""]
#[doc = "        },"]
#[doc = "        \"nullableValue\": {"]
#[doc = "          \"type\": ["]
#[doc = "            \"string\","]
#[doc = "            \"null\""]
#[doc = "          ]"]
#[doc = "        },"]
#[doc = "        \"optionalValue\": {"]
#[doc = "          \"type\": ["]
#[doc = "            \"string\","]
#[doc = "            \"null\""]
#[doc = "          ]"]
#[doc = "        },"]
#[doc = "        \"schemaVersion\": {"]
#[doc = "          \"const\": \"sapphirus.generator-qualification.v1\""]
#[doc = "        },"]
#[doc = "        \"signedValue\": {"]
#[doc = "          \"type\": \"integer\","]
#[doc = "          \"maximum\": 2147483647.0,"]
#[doc = "          \"minimum\": -2147483648.0"]
#[doc = "        },"]
#[doc = "        \"unsignedValue\": {"]
#[doc = "          \"type\": \"integer\","]
#[doc = "          \"maximum\": 4294967295.0,"]
#[doc = "          \"minimum\": 0.0"]
#[doc = "        },"]
#[doc = "        \"variant\": {"]
#[doc = "          \"oneOf\": ["]
#[doc = "            {"]
#[doc = "              \"$ref\": \"#/$defs/GeneratorQualificationTextVariant\""]
#[doc = "            },"]
#[doc = "            {"]
#[doc = "              \"$ref\": \"#/$defs/GeneratorQualificationCountVariant\""]
#[doc = "            }"]
#[doc = "          ]"]
#[doc = "        }"]
#[doc = "      },"]
#[doc = "      \"additionalProperties\": false"]
#[doc = "    }"]
#[doc = "  ]"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged, deny_unknown_fields)]
pub enum GeneratorQualification {
    WithoutOptionalValue {
        #[serde(rename = "externalValue")]
        external_value: QualificationExternalExternalValue,
        #[serde(rename = "interoperableValue")]
        interoperable_value: i64,
        items: ::std::vec::Vec<::std::string::String>,
        label: ::std::string::String,
        node: GeneratorQualificationNode,
        #[serde(rename = "nullableValue")]
        nullable_value: ::std::option::Option<::std::string::String>,
        #[serde(rename = "schemaVersion")]
        schema_version: ::serde_json::Value,
        #[serde(rename = "signedValue")]
        signed_value: i32,
        #[serde(rename = "unsignedValue")]
        unsigned_value: u32,
        variant: GeneratorQualificationWithoutOptionalValueVariant,
    },
    WithOptionalValue {
        #[serde(rename = "externalValue")]
        external_value: QualificationExternalExternalValue,
        #[serde(rename = "interoperableValue")]
        interoperable_value: i64,
        items: ::std::vec::Vec<::std::string::String>,
        label: ::std::string::String,
        node: GeneratorQualificationNode,
        #[serde(rename = "nullableValue")]
        nullable_value: ::std::option::Option<::std::string::String>,
        #[serde(rename = "optionalValue")]
        optional_value: ::std::option::Option<::std::string::String>,
        #[serde(rename = "schemaVersion")]
        schema_version: ::serde_json::Value,
        #[serde(rename = "signedValue")]
        signed_value: i32,
        #[serde(rename = "unsignedValue")]
        unsigned_value: u32,
        variant: GeneratorQualificationWithOptionalValueVariant,
    },
}
#[doc = "`GeneratorQualificationCountVariant`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"count\","]
#[doc = "    \"kind\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"count\": {"]
#[doc = "      \"type\": \"integer\""]
#[doc = "    },"]
#[doc = "    \"kind\": {"]
#[doc = "      \"const\": \"count\""]
#[doc = "    }"]
#[doc = "  },"]
#[doc = "  \"additionalProperties\": false"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct GeneratorQualificationCountVariant {
    pub count: i64,
    pub kind: ::serde_json::Value,
}
impl GeneratorQualificationCountVariant {
    pub fn builder() -> builder::GeneratorQualificationCountVariant {
        Default::default()
    }
}
#[doc = "`GeneratorQualificationNode`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"next\","]
#[doc = "    \"value\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"next\": {"]
#[doc = "      \"oneOf\": ["]
#[doc = "        {"]
#[doc = "          \"type\": \"null\""]
#[doc = "        },"]
#[doc = "        {"]
#[doc = "          \"$ref\": \"#/$defs/GeneratorQualificationNode\""]
#[doc = "        }"]
#[doc = "      ]"]
#[doc = "    },"]
#[doc = "    \"value\": {"]
#[doc = "      \"type\": \"string\""]
#[doc = "    }"]
#[doc = "  },"]
#[doc = "  \"additionalProperties\": false"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct GeneratorQualificationNode {
    pub next: ::std::option::Option<::std::boxed::Box<GeneratorQualificationNode>>,
    pub value: ::std::string::String,
}
impl GeneratorQualificationNode {
    pub fn builder() -> builder::GeneratorQualificationNode {
        Default::default()
    }
}
#[doc = "`GeneratorQualificationTextVariant`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"kind\","]
#[doc = "    \"text\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"kind\": {"]
#[doc = "      \"const\": \"text\""]
#[doc = "    },"]
#[doc = "    \"text\": {"]
#[doc = "      \"type\": \"string\""]
#[doc = "    }"]
#[doc = "  },"]
#[doc = "  \"additionalProperties\": false"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct GeneratorQualificationTextVariant {
    pub kind: ::serde_json::Value,
    pub text: ::std::string::String,
}
impl GeneratorQualificationTextVariant {
    pub fn builder() -> builder::GeneratorQualificationTextVariant {
        Default::default()
    }
}
#[doc = "`GeneratorQualificationWithOptionalValueVariant`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"oneOf\": ["]
#[doc = "    {"]
#[doc = "      \"$ref\": \"#/$defs/GeneratorQualificationTextVariant\""]
#[doc = "    },"]
#[doc = "    {"]
#[doc = "      \"$ref\": \"#/$defs/GeneratorQualificationCountVariant\""]
#[doc = "    }"]
#[doc = "  ]"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum GeneratorQualificationWithOptionalValueVariant {
    TextVariant(GeneratorQualificationTextVariant),
    CountVariant(GeneratorQualificationCountVariant),
}
impl ::std::convert::From<GeneratorQualificationTextVariant>
    for GeneratorQualificationWithOptionalValueVariant
{
    fn from(value: GeneratorQualificationTextVariant) -> Self {
        Self::TextVariant(value)
    }
}
impl ::std::convert::From<GeneratorQualificationCountVariant>
    for GeneratorQualificationWithOptionalValueVariant
{
    fn from(value: GeneratorQualificationCountVariant) -> Self {
        Self::CountVariant(value)
    }
}
#[doc = "`GeneratorQualificationWithoutOptionalValueVariant`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"oneOf\": ["]
#[doc = "    {"]
#[doc = "      \"$ref\": \"#/$defs/GeneratorQualificationTextVariant\""]
#[doc = "    },"]
#[doc = "    {"]
#[doc = "      \"$ref\": \"#/$defs/GeneratorQualificationCountVariant\""]
#[doc = "    }"]
#[doc = "  ]"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum GeneratorQualificationWithoutOptionalValueVariant {
    TextVariant(GeneratorQualificationTextVariant),
    CountVariant(GeneratorQualificationCountVariant),
}
impl ::std::convert::From<GeneratorQualificationTextVariant>
    for GeneratorQualificationWithoutOptionalValueVariant
{
    fn from(value: GeneratorQualificationTextVariant) -> Self {
        Self::TextVariant(value)
    }
}
impl ::std::convert::From<GeneratorQualificationCountVariant>
    for GeneratorQualificationWithoutOptionalValueVariant
{
    fn from(value: GeneratorQualificationCountVariant) -> Self {
        Self::CountVariant(value)
    }
}
#[doc = "`QualificationExternalExternalValue`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"object\","]
#[doc = "  \"required\": ["]
#[doc = "    \"code\""]
#[doc = "  ],"]
#[doc = "  \"properties\": {"]
#[doc = "    \"code\": {"]
#[doc = "      \"type\": \"string\","]
#[doc = "      \"pattern\": \"^ext_[A-Z0-9]{4}$\""]
#[doc = "    }"]
#[doc = "  },"]
#[doc = "  \"additionalProperties\": false"]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct QualificationExternalExternalValue {
    pub code: QualificationExternalExternalValueCode,
}
impl QualificationExternalExternalValue {
    pub fn builder() -> builder::QualificationExternalExternalValue {
        Default::default()
    }
}
#[doc = "`QualificationExternalExternalValueCode`"]
#[doc = r""]
#[doc = r" <details><summary>JSON schema</summary>"]
#[doc = r""]
#[doc = r" ```json"]
#[doc = "{"]
#[doc = "  \"type\": \"string\","]
#[doc = "  \"pattern\": \"^ext_[A-Z0-9]{4}$\""]
#[doc = "}"]
#[doc = r" ```"]
#[doc = r" </details>"]
#[derive(:: serde :: Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct QualificationExternalExternalValueCode(::std::string::String);
impl ::std::ops::Deref for QualificationExternalExternalValueCode {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<QualificationExternalExternalValueCode> for ::std::string::String {
    fn from(value: QualificationExternalExternalValueCode) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for QualificationExternalExternalValueCode {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
            ::std::sync::LazyLock::new(|| ::regress::Regex::new("^ext_[A-Z0-9]{4}$").unwrap());
        if PATTERN.find(value).is_none() {
            return Err("doesn't match pattern \"^ext_[A-Z0-9]{4}$\"".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for QualificationExternalExternalValueCode {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for QualificationExternalExternalValueCode {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for QualificationExternalExternalValueCode {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for QualificationExternalExternalValueCode {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|e: self::error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(e.to_string())
            })
    }
}
#[doc = r" Types for composing complex structures."]
pub mod builder {
    #[derive(Clone, Debug)]
    pub struct GeneratorQualificationCountVariant {
        count: ::std::result::Result<i64, ::std::string::String>,
        kind: ::std::result::Result<::serde_json::Value, ::std::string::String>,
    }
    impl ::std::default::Default for GeneratorQualificationCountVariant {
        fn default() -> Self {
            Self {
                count: Err("no value supplied for count".to_string()),
                kind: Err("no value supplied for kind".to_string()),
            }
        }
    }
    impl GeneratorQualificationCountVariant {
        pub fn count<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<i64>,
            T::Error: ::std::fmt::Display,
        {
            self.count = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for count: {e}"));
            self
        }
        pub fn kind<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::serde_json::Value>,
            T::Error: ::std::fmt::Display,
        {
            self.kind = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for kind: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<GeneratorQualificationCountVariant>
        for super::GeneratorQualificationCountVariant
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: GeneratorQualificationCountVariant,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                count: value.count?,
                kind: value.kind?,
            })
        }
    }
    impl ::std::convert::From<super::GeneratorQualificationCountVariant>
        for GeneratorQualificationCountVariant
    {
        fn from(value: super::GeneratorQualificationCountVariant) -> Self {
            Self {
                count: Ok(value.count),
                kind: Ok(value.kind),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct GeneratorQualificationNode {
        next: ::std::result::Result<
            ::std::option::Option<::std::boxed::Box<super::GeneratorQualificationNode>>,
            ::std::string::String,
        >,
        value: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for GeneratorQualificationNode {
        fn default() -> Self {
            Self {
                next: Err("no value supplied for next".to_string()),
                value: Err("no value supplied for value".to_string()),
            }
        }
    }
    impl GeneratorQualificationNode {
        pub fn next<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<
                ::std::option::Option<::std::boxed::Box<super::GeneratorQualificationNode>>,
            >,
            T::Error: ::std::fmt::Display,
        {
            self.next = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for next: {e}"));
            self
        }
        pub fn value<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.value = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for value: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<GeneratorQualificationNode> for super::GeneratorQualificationNode {
        type Error = super::error::ConversionError;
        fn try_from(
            value: GeneratorQualificationNode,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                next: value.next?,
                value: value.value?,
            })
        }
    }
    impl ::std::convert::From<super::GeneratorQualificationNode> for GeneratorQualificationNode {
        fn from(value: super::GeneratorQualificationNode) -> Self {
            Self {
                next: Ok(value.next),
                value: Ok(value.value),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct GeneratorQualificationTextVariant {
        kind: ::std::result::Result<::serde_json::Value, ::std::string::String>,
        text: ::std::result::Result<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for GeneratorQualificationTextVariant {
        fn default() -> Self {
            Self {
                kind: Err("no value supplied for kind".to_string()),
                text: Err("no value supplied for text".to_string()),
            }
        }
    }
    impl GeneratorQualificationTextVariant {
        pub fn kind<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::serde_json::Value>,
            T::Error: ::std::fmt::Display,
        {
            self.kind = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for kind: {e}"));
            self
        }
        pub fn text<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<::std::string::String>,
            T::Error: ::std::fmt::Display,
        {
            self.text = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for text: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<GeneratorQualificationTextVariant>
        for super::GeneratorQualificationTextVariant
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: GeneratorQualificationTextVariant,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self {
                kind: value.kind?,
                text: value.text?,
            })
        }
    }
    impl ::std::convert::From<super::GeneratorQualificationTextVariant>
        for GeneratorQualificationTextVariant
    {
        fn from(value: super::GeneratorQualificationTextVariant) -> Self {
            Self {
                kind: Ok(value.kind),
                text: Ok(value.text),
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct QualificationExternalExternalValue {
        code: ::std::result::Result<
            super::QualificationExternalExternalValueCode,
            ::std::string::String,
        >,
    }
    impl ::std::default::Default for QualificationExternalExternalValue {
        fn default() -> Self {
            Self {
                code: Err("no value supplied for code".to_string()),
            }
        }
    }
    impl QualificationExternalExternalValue {
        pub fn code<T>(mut self, value: T) -> Self
        where
            T: ::std::convert::TryInto<super::QualificationExternalExternalValueCode>,
            T::Error: ::std::fmt::Display,
        {
            self.code = value
                .try_into()
                .map_err(|e| format!("error converting supplied value for code: {e}"));
            self
        }
    }
    impl ::std::convert::TryFrom<QualificationExternalExternalValue>
        for super::QualificationExternalExternalValue
    {
        type Error = super::error::ConversionError;
        fn try_from(
            value: QualificationExternalExternalValue,
        ) -> ::std::result::Result<Self, super::error::ConversionError> {
            Ok(Self { code: value.code? })
        }
    }
    impl ::std::convert::From<super::QualificationExternalExternalValue>
        for QualificationExternalExternalValue
    {
        fn from(value: super::QualificationExternalExternalValue) -> Self {
            Self {
                code: Ok(value.code),
            }
        }
    }
}
