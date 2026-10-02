
SHOWN = 0
function StartEraShow() SHOWN = SHOWN + 1 end
function OnCheckGameEraChanged() StartEraShow() end
