//! Exact Lua 5.1 value comparisons for generated, read-only save diagnostics.
//! Numbers use round-trip precision; addresses and executable values are never
//! treated as stable evidence. Type/length prefixes keep strings unambiguous.
pub const SERIAL: &str = r#"
local function scalar(v)
 local kind=type(v); local text
 if kind=='number' then
  if v~=v or v==math.huge or v==-math.huge then error('nonfinite snapshot number') end
  text=v==0 and '0' or string.format('%.17g',v)
 elseif kind=='string' then text=v
 elseif kind=='boolean' or kind=='nil' then text=tostring(v)
 else error('unsupported snapshot value type: '..kind) end
 return kind..':'..#text..':'..text
end
local function serial(v,path)
 if type(v)~='table' then return scalar(v) end
 path=path or {}; if path[v] then error('cyclic snapshot table') end; path[v]=true
 local keys={}
 for key in pairs(v) do
  local kind=type(key)
  if kind~='number' and kind~='string' and kind~='boolean' then error('unstable snapshot key type: '..kind) end
  keys[#keys+1]={key=key,encoded=scalar(key)}
 end
 table.sort(keys,function(a,b) return a.encoded<b.encoded end)
 local out={}
 for _,entry in ipairs(keys) do
  out[#out+1]=entry.encoded..'='..serial(rawget(v,entry.key),path)
 end
 path[v]=nil
 return '{'..table.concat(out,';')..'}'
end
"#;
