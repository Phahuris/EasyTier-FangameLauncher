# FGL_Trade v1 — EN

module FGLTrade
  DIR = "FGL_peers"
  TICK = 0.25
  COOLDOWN = 3.0
  @last = 0.0
  @busy = false
  @active = false
  @last_sent = 0.0
  @hooks = false
  @trade_id = nil
  @remote_id = nil
  @remote_name = nil
  @my_offer_idx = nil

  def self.log(m); end
  def self.busy?; @busy || @active; end
  def self.active?; @active; end

  def self.my_id
    FGL.instance_variable_get(:@my_id) rescue nil
  end

  def self.my_name
    (FGL.player_display_name rescue nil) || ($Trainer.name.to_s rescue "Player")
  end

  def self.ensure_dir
    Dir.mkdir(DIR) unless File.directory?(DIR) rescue nil
  end

  def self.write_text(name, text)
    ensure_dir
    File.open(File.join(DIR, name), "w") { |f| f.write(text.to_s) } rescue nil
  end

  def self.read_text(name)
    path = File.join(DIR, name)
    return nil unless File.exist?(path)
    File.read(path) rescue nil
  end

  def self.write_bin(name, obj)
    ensure_dir
    File.open(File.join(DIR, name), "wb") { |f| Marshal.dump(obj, f) } rescue nil
  end

  def self.read_bin(name)
    path = File.join(DIR, name)
    return nil unless File.exist?(path)
    File.open(path, "rb") { |f| Marshal.load(f) } rescue nil
  end

  def self.delete_file(name)
    path = File.join(DIR, name)
    File.delete(path) if File.exist?(path) rescue nil
  end

  def self.force_msg_bottom_system!
    begin; $game_system.message_position = 2 if $game_system; rescue; end
  end

  def self.force_save_play_slot
    begin
      slot = nil
      slot = $Trainer.save_slot if $Trainer && $Trainer.respond_to?(:save_slot)
      if defined?(Game) && Game.respond_to?(:save) && !slot.nil?
        begin; Game.save(slot, true); rescue; Game.save(slot) rescue nil; end
      else
        Kernel.pbSave(false) rescue nil
      end
    rescue; end
  end

  def self.show_prep_phase(text = nil)
    force_msg_bottom_system!
    text = _INTL("Preparing the trade...") if text.nil?
    msg = nil
    begin
      msg = pbCreateMessageWindow
      msg.letterbyletter = false
      msg.text = text.to_s
      pbRepositionMessageWindow(msg, 2) rescue nil
      msg.y = Graphics.height - msg.height if msg.respond_to?(:height)
    rescue; msg = nil; end
    20.times do
      msg.update rescue nil
      Graphics.update rescue nil
      Input.update rescue nil
    end
    pbDisposeMessageWindow(msg) rescue nil
  end

  def self.resolve_name(id, data = nil)
    n = nil
    if data
      begin
        n = data[:name] || data["name"] || data[:display_name] || data["display_name"] || data[:pname]
      rescue; end
    end
    if n.nil? || n.to_s == "" || n.to_s =~ /^\d{5,}/
      begin
        path = File.join(DIR, "#{id}.txt")
        if File.exist?(path)
          raw = File.read(path) rescue ""
          parts = raw.to_s.split("|")
          n = parts[16] if parts.size > 16
        end
      rescue; end
    end
    if n.nil? || n.to_s == "" || n.to_s =~ /^\d{5,}/
      n = _INTL("Player")
    end
    n.to_s
  end

  def self.all_connected_peers
    list = []
    begin
      players = FGL.instance_variable_get(:@players) rescue nil
      return list unless players.is_a?(Hash)
      mid = my_id.to_s
      players.each do |id, data|
        next if id.to_s == mid
        list << [id, resolve_name(id, data)]
      end
    rescue; end
    list
  end

  def self.find_peer_by_name(name)
    name_l = name.to_s.downcase.strip
    all_connected_peers.each do |id, pname|
      return [id, pname] if pname.to_s.downcase.strip == name_l
    end
    nil
  end

  def self.peer_is_busy?(peer_id)
    return true if peer_id.nil?
    ensure_dir
    Dir.glob(File.join(DIR, "trd_*")).each do |f|
      begin
        raw = File.read(f) rescue next
        if raw.include?(peer_id.to_s) || File.basename(f).include?(peer_id.to_s)
          age = Time.now - File.mtime(f) rescue 999
          return true if age < 30.0
        end
      rescue; end
    end
    false
  end

  def self.can_send_trade?(target_id)
    now = Time.now.to_f rescue 0.0
    @last_sent = 0.0 if @last_sent.nil?
    if (now - @last_sent) < COOLDOWN
      return [false, _INTL("Please wait a moment before sending another trade request.")]
    end
    if busy?
      return [false, _INTL("You are already in a trade.")]
    end
    begin
      if defined?(FGLBattle) && (FGLBattle.busy? || FGLBattle.active?)
        return [false, _INTL("You are already in a battle.")]
      end
    rescue; end
    if peer_is_busy?(target_id)
      return [false, _INTL("This person is busy or already has a pending request.")]
    end
    [true, nil]
  end

  def self.choose_party_index
    chosen = nil
    begin
      pbFadeOutIn(99999) {
        scene = PokemonParty_Scene.new
        screen = PokemonPartyScreen.new(scene, $Trainer.party)
        screen.pbStartScene(_INTL("Choose a Pokémon to trade."), false)
        loop do
          pkmnid = screen.pbChoosePokemon
          if pkmnid.nil? || pkmnid < 0
            chosen = nil
            break
          end
          pkmn = $Trainer.party[pkmnid]
          next if pkmn.nil?
          cmds = [_INTL("Trade"), _INTL("Summary"), _INTL("Back")]
          cmd = -1
          begin
            cmd = scene.pbShowCommands(_INTL("What to do with {1}?", pkmn.name), cmds)
          rescue
            begin
              cmd = Kernel.pbMessage(_INTL("What to do with {1}?", pkmn.name), cmds, 2)
            rescue
              cmd = 0
            end
          end
          if cmd == 0
            chosen = pkmnid
            break
          elsif cmd == 1
            begin
              screen.pbSummary(pkmnid)
            rescue
              scene.pbSummary(pkmnid) rescue nil
            end
          end
        end
        screen.pbEndScene
      }
    rescue Exception => e
      begin
        pbChoosePokemon(1, 3)
        idx = pbGet(1)
        chosen = (idx && idx >= 0) ? idx : nil
      rescue
        chosen = nil
      end
    end
    chosen
  end

  def self.cleanup
    @busy = false
    @active = false
    @trade_id = nil
    @remote_id = nil
    @remote_name = nil
    @my_offer_idx = nil
    mid = my_id.to_s
    Dir.glob(File.join(DIR, "trd_*")).each do |f|
      begin
        raw = File.read(f) rescue ""
        File.delete(f) if raw.include?(mid) || File.basename(f).include?(mid)
      rescue; end
    end
  end

  def self.poll_commands
    mid = my_id
    return if mid.nil?
    raw = read_text("cmd_#{mid}.txt")
    return if raw.nil? || raw.to_s.strip == ""
    line = raw.to_s.strip
    if line =~ /^\/trade\s+(.+)$/i
      delete_file("cmd_#{mid}.txt")
      trade_player($1.strip)
    elsif line =~ /^\/battle\s+/i
      # laisse Battle gerer
    else
      delete_file("cmd_#{mid}.txt")
    end
  end

  def self.trade_player(target_name)
    return if busy?
    peer = find_peer_by_name(target_name)
    if peer.nil?
      force_msg_bottom_system!
      pbMessage("\\wd" + _INTL("Player introuvable : {1}", target_name)) rescue nil
      return
    end
    send_request(peer[0], peer[1])
  end

  def self.send_request(target_id, target_name)
    ok, msg = can_send_trade?(target_id)
    unless ok
      force_msg_bottom_system!
      pbMessage("\\wd" + msg.to_s) rescue nil
      return false
    end
    @last_sent = Time.now.to_f
    @busy = true
    tid = "T#{(Time.now.to_f * 1000).to_i}_#{my_id}"
    @trade_id = tid
    @remote_id = target_id
    @remote_name = target_name.to_s
    write_text("trd_req_#{tid}.txt", "#{my_id}|#{target_id}|#{my_name}|REQ")
    force_msg_bottom_system!
    pbMessage("\\wd" + _INTL("Trade request sent to {1}.", target_name)) rescue nil
    true
  end

  def self.accept_request(tid, from_id, from_name)
    return if busy?
    begin
      if defined?(FGLBattle) && (FGLBattle.busy? || FGLBattle.active?)
        write_text("trd_ans_#{tid}.txt", "#{my_id}|#{from_id}|REFUSE")
        return
      end
    rescue; end
    force_msg_bottom_system!
    ok = false
    begin
      ok = pbConfirmMessage("\\wd" + _INTL("{1} veut Trade un Pokemon. Accepter ?", from_name))
    rescue
      ok = true
    end
    if ok
      write_text("trd_ans_#{tid}.txt", "#{my_id}|#{from_id}|ACCEPT")
      @busy = true
      @active = true
      @trade_id = tid
      @remote_id = from_id
      @remote_name = from_name
      start_trade_flow(false)
    else
      write_text("trd_ans_#{tid}.txt", "#{my_id}|#{from_id}|REFUSE")
    end
  end

  def self.start_trade_flow(_initiator)
    show_prep_phase(_INTL("Preparing the trade..."))
    idx = choose_party_index
    if idx.nil?
      force_msg_bottom_system!
      pbMessage("\\wd" + _INTL("Trade cancelled.")) rescue nil
      write_text("trd_cancel_#{@trade_id}.txt", "#{my_id}|CANCEL")
      cleanup
      return
    end
    @my_offer_idx = idx
    pkmn = $Trainer.party[idx]
    write_bin("trd_offer_#{@trade_id}_#{my_id}.bin", pkmn)
    show_prep_phase(_INTL("Loading the trade..."))
    remote_pkmn = nil
    150.times do
      Graphics.update rescue nil
      Input.update rescue nil
      remote_pkmn = read_bin("trd_offer_#{@trade_id}_#{@remote_id}.bin")
      break if remote_pkmn
    end
    if remote_pkmn.nil?
      force_msg_bottom_system!
      pbMessage("\\wd" + _INTL("L'autre Player n'a pas repondu a temps.")) rescue nil
      cleanup
      return
    end
    show_prep_phase(_INTL("Trade in progress..."))
    write_text("trd_confirm_#{@trade_id}_#{my_id}.txt", "OK")
    confirmed = false
    90.times do
      Graphics.update rescue nil
      Input.update rescue nil
      if read_text("trd_confirm_#{@trade_id}_#{@remote_id}.txt").to_s.include?("OK")
        confirmed = true
        break
      end
    end
    unless confirmed
      force_msg_bottom_system!
      pbMessage("\\wd" + _INTL("Trade not confirmed.")) rescue nil
      cleanup
      return
    end
    given = nil
    begin
      given = $Trainer.party[@my_offer_idx]
      $Trainer.party[@my_offer_idx] = remote_pkmn
      begin
        remote_pkmn.obtainMode = 2 if remote_pkmn.respond_to?(:obtainMode=)
      rescue; end
      begin
        $Trainer.seen[remote_pkmn.species] = true if $Trainer.respond_to?(:seen)
        $Trainer.owned[remote_pkmn.species] = true if $Trainer.respond_to?(:owned)
        pbSeenForm(remote_pkmn) rescue nil
      rescue; end
    rescue Exception => e
      cleanup
      return
    end
    force_save_play_slot
    force_msg_bottom_system!
    begin
      if defined?(PokemonTrade_Scene) || defined?(PokemonTradeScene)
        pbFadeOutInWithMusic(99999) {
          force_save_play_slot
          scene = (defined?(PokemonTrade_Scene) ? PokemonTrade_Scene : PokemonTradeScene).new
          scene.pbStartScreen(given, remote_pkmn, my_name, @remote_name.to_s) rescue nil
          scene.pbTrade rescue nil
          force_save_play_slot
          scene.pbEndScreen rescue nil
        }
      else
        pbMessage("\\wd" + _INTL("You traded {1} for {2} from {3}!",
          (given.name rescue "Pokemon"),
          (remote_pkmn.name rescue "Pokemon"),
          @remote_name.to_s)) rescue nil
      end
    rescue
      pbMessage("\\wd" + _INTL("Trade completed!")) rescue nil
    end
    force_save_play_slot
    cleanup
  end

  def self.tick
    now = Time.now.to_f rescue 0.0
    return if now - @last < TICK
    @last = now
    mid = my_id
    return if mid.nil?
    begin; poll_commands; rescue; end
    if @busy && !@active && @trade_id
      ans = read_text("trd_ans_#{@trade_id}.txt")
      if ans
        if ans.include?("ACCEPT")
          @active = true
          start_trade_flow(true)
        elsif ans.include?("REFUSE")
          force_msg_bottom_system!
          pbMessage("\\wd" + _INTL("Trade request declined.")) rescue nil
          cleanup
        end
      end
      age = 0
      begin
        f = File.join(DIR, "trd_req_#{@trade_id}.txt")
        age = Time.now - File.mtime(f) if File.exist?(f)
      rescue; end
      if age > 30
        force_msg_bottom_system!
        pbMessage("\\wd" + _INTL("Trade request expired.")) rescue nil
        cleanup
      end
      return
    end
    return if busy?
    battle_busy = false
    begin
      battle_busy = true if defined?(FGLBattle) && (FGLBattle.busy? || FGLBattle.active?)
    rescue; end
    return if battle_busy
    Dir.glob(File.join(DIR, "trd_req_*.txt")).each do |f|
      begin
        raw = File.read(f) rescue next
        parts = raw.strip.split("|")
        next if parts.length < 4
        from_id = parts[0]
        to_id = parts[1]
        from_name = parts[2]
        kind = parts[3]
        next unless to_id.to_s == mid.to_s
        next unless kind == "REQ"
        tid = File.basename(f).sub("trd_req_", "").sub(".txt", "")
        next if read_text("trd_ans_#{tid}.txt")
        age = Time.now - File.mtime(f) rescue 999
        next if age > 30
        accept_request(tid, from_id, from_name)
        break
      rescue; end
    end
  end

  def self.install!
    return if @hooks
    @hooks = true
  end
end

class Scene_Map
  unless method_defined?(:_fgltrade_update)
    alias_method :_fgltrade_update, :update
  end
  def update
    _fgltrade_update
    FGLTrade.tick rescue nil
  end
end

FGLTrade.install! rescue nil