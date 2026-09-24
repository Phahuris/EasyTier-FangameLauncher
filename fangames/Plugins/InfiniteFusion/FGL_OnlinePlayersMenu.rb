#===============================================================================
# FGL_OnlinePlayersMenu
# Pause -> Online -> liste (RemotePlayer) -> Battle / Trade / Cancel
# Ne modifie aucun autre fichier du jeu. Bridge find_peer vers @remotes.
#===============================================================================

module FGLOnlinePlayersMenu
  BG_PATH = "Graphics/Pictures/fgl_online_bg"

  def self.my_id
    begin
      if defined?(FGL_RemotePlayer_Test)
        id = FGL_RemotePlayer_Test.instance_variable_get(:@my_id) rescue nil
        return id.to_s if id && id.to_s != ""
      end
    rescue; end
    begin
      if defined?(FGL)
        id = FGL.instance_variable_get(:@my_id) rescue nil
        return id.to_s if id && id.to_s != ""
      end
    rescue; end
    ""
  end

  def self.collect_peers
    list = []
    mid = my_id
    seen = {}
    begin
      if defined?(FGL_RemotePlayer_Test)
        rem = FGL_RemotePlayer_Test.instance_variable_get(:@remotes) rescue nil
        if rem.is_a?(Hash)
          rem.each do |id, rec|
            next if id.nil?
            sid = id.to_s
            next if sid == "" || sid == mid || seen[sid]
            name = ""
            begin; name = (rec[:pname] || rec[:name] || "").to_s; rescue; end
            name = "Player #{sid}" if name == ""
            list << { :id => sid, :name => name }
            seen[sid] = true
          end
        end
      end
    rescue; end
    begin
      if defined?(FGL)
        players = FGL.instance_variable_get(:@players) rescue nil
        if players.is_a?(Hash)
          players.each do |id, data|
            next if id.nil?
            sid = id.to_s
            next if sid == "" || sid == mid || seen[sid]
            name = ""
            begin; name = (data[:pname] || data[:name] || "").to_s; rescue; end
            name = "Player #{sid}" if name == ""
            list << { :id => sid, :name => name }
            seen[sid] = true
          end
        end
      end
    rescue; end
    list.sort_by { |e| e[:name].to_s.downcase }
  end

  def self.open_from_pause
    peers = collect_peers
    if peers.empty?
      pbMessage(_INTL("No players online for this game.")) rescue nil
      return false
    end
    pbFadeOutIn(99999) {
      scene = FGLOnlinePlayersMenu_Scene.new
      scene.pbStartScene(peers)
      scene.pbMain
      scene.pbEndScene
    }
    true
  end
end

#-------------------------------------------------------------------------------
# Bridge: FGLBattle / FGLTrade cherchaient seulement FGL.@players
#-------------------------------------------------------------------------------
if defined?(FGLBattle)
  module FGLBattle
    class << self
      unless method_defined?(:fgl_opm_find_peer_by_name)
        if method_defined?(:find_peer_by_name)
          alias fgl_opm_find_peer_by_name find_peer_by_name
        end
      end
    end

    def self.find_peer_by_name(name)
      name_l = name.to_s.downcase.strip
      begin
        if defined?(FGL_RemotePlayer_Test)
          rem = FGL_RemotePlayer_Test.instance_variable_get(:@remotes) rescue nil
          if rem.is_a?(Hash)
            rem.each do |id, rec|
              pname = (rec[:pname] || rec[:name] || "").to_s
              next if pname.downcase.strip != name_l
              outfit = {}
              begin
                outfit = {
                  :clothes => (rec[:clothes] || "").to_s,
                  :hair => (rec[:hair] || "").to_s,
                  :hat => (rec[:hat] || "").to_s,
                  :hat2 => (rec[:hat2] || "").to_s,
                  :clothes_color => (rec[:cc] || 0).to_i,
                  :hair_color => (rec[:hc] || 0).to_i,
                  :hat_color => (rec[:htc] || 0).to_i,
                  :hat2_color => (rec[:h2c] || 0).to_i,
                  :skin_color => (rec[:skin] || 0).to_i
                }
              rescue; end
              return [id, pname, outfit]
            end
          end
        end
      rescue; end
      if respond_to?(:fgl_opm_find_peer_by_name)
        return fgl_opm_find_peer_by_name(name)
      end
      nil
    end
  end
end

if defined?(FGLTrade)
  module FGLTrade
    class << self
      unless method_defined?(:fgl_opm_find_peer_by_name)
        if method_defined?(:find_peer_by_name)
          alias fgl_opm_find_peer_by_name find_peer_by_name
        end
      end
    end

    def self.find_peer_by_name(name)
      name_l = name.to_s.downcase.strip
      begin
        if defined?(FGL_RemotePlayer_Test)
          rem = FGL_RemotePlayer_Test.instance_variable_get(:@remotes) rescue nil
          if rem.is_a?(Hash)
            rem.each do |id, rec|
              pname = (rec[:pname] || rec[:name] || "").to_s
              next if pname.downcase.strip != name_l
              return [id, pname]
            end
          end
        end
      rescue; end
      if respond_to?(:fgl_opm_find_peer_by_name)
        return fgl_opm_find_peer_by_name(name)
      end
      nil
    end
  end
end

#-------------------------------------------------------------------------------
class FGLOnlinePlayersMenu_Scene
  def pbStartScene(peers)
    @peers = peers
    @viewport = Viewport.new(0, 0, Graphics.width, Graphics.height)
    @viewport.z = 99999
    @sprites = {}

    if pbResolveBitmap(FGLOnlinePlayersMenu::BG_PATH)
      @sprites["bg"] = IconSprite.new(0, 0, @viewport)
      @sprites["bg"].setBitmap(FGLOnlinePlayersMenu::BG_PATH)
    else
      @sprites["bg"] = BitmapSprite.new(Graphics.width, Graphics.height, @viewport)
      @sprites["bg"].bitmap.fill_rect(0, 0, Graphics.width, Graphics.height, Color.new(12, 12, 20))
    end

    # Hauteur 64+ pour que le skin + texte ne rognent pas
    @sprites["title"] = Window_UnformattedTextPokemon.newWithSize(
      _INTL("Online players"), 0, 0, Graphics.width, 64, @viewport
    )
    begin
      @sprites["title"].windowskin = nil  # optionnel: moins de bordure qui mange la hauteur
    rescue
    end

    names = @peers.map { |p| p[:name].to_s }
    @sprites["list"] = Window_CommandPokemon.new(names)
    @sprites["list"].viewport = @viewport
    @sprites["list"].x = 32
    @sprites["list"].y = 68
    begin
      @sprites["list"].width  = Graphics.width - 64
      @sprites["list"].height = Graphics.height - 68 - 68
    rescue
    end

    @sprites["help"] = Window_UnformattedTextPokemon.newWithSize(
      _INTL("Z/C: Select    X: Back"), 0, Graphics.height - 64, Graphics.width, 64, @viewport
    )
  end


  def pbMain
    loop do
      Graphics.update
      Input.update
      @sprites["list"].update
      if Input.trigger?(Input::BACK)
        pbPlayCloseMenuSE rescue nil
        break
      elsif Input.trigger?(Input::USE)
        idx = @sprites["list"].index
        next if idx.nil? || idx < 0 || idx >= @peers.length
        peer = @peers[idx]
        pbPlayDecisionSE rescue nil
        action = pbSubMenu(peer)
        if action == :battle || action == :trade
          pbSendInvite(peer, action)
          break
        end
      end
    end
  end

  def pbSubMenu(peer)
    cmds = [_INTL("Battle"), _INTL("Trade"), _INTL("Cancel")]
    cmd = -1
    begin
      cmd = pbMessage(_INTL("Invite {1}?", peer[:name]), cmds, 2)
    rescue
      begin
        cmd = Kernel.pbMessage(_INTL("Invite {1}?", peer[:name]), cmds, 2)
      rescue
        cmd = 2
      end
    end
    return :battle if cmd == 0
    return :trade if cmd == 1
    :cancel
  end

  def pbSendInvite(peer, action)
    name = peer[:name].to_s
    begin; $game_temp.in_menu = false if $game_temp; rescue; end
    if action == :battle
      if defined?(FGLBattle) && FGLBattle.respond_to?(:challenge_player)
        FGLBattle.challenge_player(name)
      else
        pbMessage(_INTL("Battle module unavailable.")) rescue nil
      end
    elsif action == :trade
      if defined?(FGLTrade) && FGLTrade.respond_to?(:trade_player)
        FGLTrade.trade_player(name)
      else
        pbMessage(_INTL("Trade module unavailable.")) rescue nil
      end
    end
  end

  def pbEndScene
    pbDisposeSpriteHash(@sprites) rescue nil
    @viewport.dispose if @viewport
  end
end

#-------------------------------------------------------------------------------
# Injection PauseMenu (classe rouvverte ici uniquement — fichier jeu non edite)
#-------------------------------------------------------------------------------
class PokemonPauseMenu
  alias fgl_opm_pbStartPokemonMenu pbStartPokemonMenu unless method_defined?(:fgl_opm_pbStartPokemonMenu)

  def pbStartPokemonMenu
    if !$Trainer
      fgl_opm_pbStartPokemonMenu
      return
    end
    @scene.pbStartScene
    endscene = true
    commands = []
    cmdPokedex = -1; cmdPokemon = -1; cmdBag = -1; cmdTrainer = -1
    cmdOutfit = -1; cmdSave = -1; cmdOption = -1; cmdPokegear = -1
    cmdOnline = -1; cmdDebug = -1; cmdQuit = -1; cmdEndGame = -1

    if $Trainer.has_pokedex && $Trainer.pokedex.accessible_dexes.length > 0
      commands[cmdPokedex = commands.length] = _INTL("Pokédex")
    end
    commands[cmdPokemon = commands.length] = _INTL("Pokémon") if $Trainer.party_count > 0
    commands[cmdBag = commands.length] = _INTL("Bag") if !pbInBugContest?
    commands[cmdPokegear = commands.length] = _INTL("Pokégear") if $Trainer.has_pokegear
    commands[cmdOnline = commands.length] = _INTL("Online")
    commands[cmdTrainer = commands.length] = $Trainer.name
    commands[cmdOutfit = commands.length] = _INTL("Outfit") if $Trainer.respond_to?(:can_change_outfit) && $Trainer.can_change_outfit
    if pbInSafari?
      if Settings::SAFARI_STEPS <= 0
        @scene.pbShowInfo(_INTL("Balls: {1}", pbSafariState.ballcount))
      else
        @scene.pbShowInfo(_INTL("Steps: {1}/{2}\nBalls: {3}",
          pbSafariState.steps, Settings::SAFARI_STEPS, pbSafariState.ballcount))
      end
      commands[cmdQuit = commands.length] = _INTL("Quit")
    elsif pbInBugContest?
      if pbBugContestState.lastPokemon
        @scene.pbShowInfo(_INTL("Caught: {1}\nLevel: {2}\nBalls: {3}",
          pbBugContestState.lastPokemon.speciesName,
          pbBugContestState.lastPokemon.level, pbBugContestState.ballcount))
      else
        @scene.pbShowInfo(_INTL("Caught: None\nBalls: {1}", pbBugContestState.ballcount))
      end
      commands[cmdQuit = commands.length] = _INTL("Quit Contest")
    else
      commands[cmdSave = commands.length] = _INTL("Save") if $game_system && !$game_system.save_disabled
    end
    commands[cmdOption = commands.length] = _INTL("Options")
    commands[cmdDebug = commands.length] = _INTL("Debug") if $DEBUG
    commands[cmdEndGame = commands.length] = _INTL("Title screen")

    loop do
      command = @scene.pbShowCommands(commands)
      if cmdOnline >= 0 && command == cmdOnline
        pbPlayDecisionSE
        close_pause = FGLOnlinePlayersMenu.open_from_pause
        if close_pause
          @scene.pbEndScene
          endscene = false
          break
        else
          @scene.pbRefresh rescue nil
          @scene.pbShowMenu rescue nil
        end
      elsif cmdPokedex >= 0 && command == cmdPokedex
        pbPlayDecisionSE
        if Settings::USE_CURRENT_REGION_DEX
          pbFadeOutIn {
            scene = PokemonPokedex_Scene.new
            screen = PokemonPokedexScreen.new(scene)
            screen.pbStartScreen
            @scene.pbRefresh
          }
        else
          $PokemonGlobal.pokedexDex = $Trainer.pokedex.accessible_dexes[0]
          pbFadeOutIn {
            scene = PokemonPokedexMenu_Scene.new
            screen = PokemonPokedexMenuScreen.new(scene)
            screen.pbStartScreen
            @scene.pbRefresh
          }
        end
      elsif cmdPokemon >= 0 && command == cmdPokemon
        pbPlayDecisionSE
        hiddenmove = nil
        pbFadeOutIn {
          sscene = PokemonParty_Scene.new
          sscreen = PokemonPartyScreen.new(sscene, $Trainer.party)
          hiddenmove = sscreen.pbPokemonScreen
          (hiddenmove) ? @scene.pbEndScene : @scene.pbRefresh
        }
        if hiddenmove
          $game_temp.in_menu = false
          pbUseHiddenMove(hiddenmove[0], hiddenmove[1])
          return
        end
      elsif cmdBag >= 0 && command == cmdBag
        pbPlayDecisionSE
        item = nil
        pbFadeOutIn {
          scene = PokemonBag_Scene.new
          screen = PokemonBagScreen.new(scene, $PokemonBag)
          item = screen.pbStartScreen
          (item) ? @scene.pbEndScene : @scene.pbRefresh
        }
        if item
          $game_temp.in_menu = false
          pbUseKeyItemInField(item)
          return
        end
      elsif cmdPokegear >= 0 && command == cmdPokegear
        pbPlayDecisionSE
        pbFadeOutIn {
          scene = PokemonPokegear_Scene.new
          screen = PokemonPokegearScreen.new(scene)
          screen.pbStartScreen
          @scene.pbRefresh
        }
      elsif cmdTrainer >= 0 && command == cmdTrainer
        pbPlayDecisionSE
        pbFadeOutIn {
          scene = PokemonTrainerCard_Scene.new
          screen = PokemonTrainerCardScreen.new(scene)
          screen.pbStartScreen
          @scene.pbRefresh
        }
      elsif cmdOutfit && cmdOutfit >= 0 && command == cmdOutfit
        @scene.pbHideMenu
        pbCommonEvent(COMMON_EVENT_OUTFIT) if defined?(COMMON_EVENT_OUTFIT)
      elsif cmdQuit >= 0 && command == cmdQuit
        @scene.pbHideMenu
        if pbInSafari?
          if pbConfirmMessage(_INTL("Would you like to leave the Safari Game right now?"))
            @scene.pbEndScene; pbSafariState.decision = 1; pbSafariState.pbGoToStart; return
          else; pbShowMenu; end
        else
          if pbConfirmMessage(_INTL("Would you like to end the Contest now?"))
            @scene.pbEndScene; pbBugContestState.pbStartJudging; return
          else; pbShowMenu; end
        end
      elsif cmdSave >= 0 && command == cmdSave
        @scene.pbHideMenu
        scene = PokemonSave_Scene.new
        screen = PokemonSaveScreen.new(scene)
        if screen.pbSaveScreen
          @scene.pbEndScene; endscene = false; break
        else; pbShowMenu; end
      elsif cmdOption >= 0 && command == cmdOption
        pbPlayDecisionSE
        pbFadeOutIn {
          scene = PokemonGameOption_Scene.new
          screen = PokemonOptionScreen.new(scene)
          screen.pbStartScreen
          pbUpdateSceneMap
          @scene.pbRefresh
        }
      elsif cmdDebug >= 0 && command == cmdDebug
        pbPlayDecisionSE
        pbFadeOutIn { pbDebugMenu; @scene.pbRefresh }
      elsif cmdEndGame >= 0 && command == cmdEndGame
        @scene.pbHideMenu
        if pbConfirmMessage(_INTL("Are you sure you want to quit the game and return to the main menu?"))
          scene = PokemonSave_Scene.new
          screen = PokemonSaveScreen.new(scene)
          screen.pbSaveScreen
          $game_temp.to_title = true
          return
        else; pbShowMenu; end
      else
        pbPlayCloseMenuSE
        break
      end
    end
    @scene.pbEndScene if endscene
  end
end