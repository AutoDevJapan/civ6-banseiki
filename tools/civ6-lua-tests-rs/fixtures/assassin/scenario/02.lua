local function Same(a,b)
      if type(a)~=type(b) then return false end
      if type(a)~='table' then return a==b end
      for k,v in pairs(a) do if not Same(v,b[k]) then return false end end
      for k,v in pairs(b) do if not Same(v,a[k]) then return false end end
      return true
    end;␠