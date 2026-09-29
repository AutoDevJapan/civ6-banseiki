-- Static map: terrain and starts are stored in the Civ6Map file.
function InitializeNewGame() end
LuaEvents.NewGameInitialized.Add(InitializeNewGame);
