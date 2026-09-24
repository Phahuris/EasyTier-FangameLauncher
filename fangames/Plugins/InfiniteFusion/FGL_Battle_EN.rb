# FGL_Battle v1 — EN

module FGLBattle
  DIR = "FGL_peers"
  TICK = 0.12
  BATTLE_BGM = "FGL_Battle"
  CHAL_COOLDOWN = 4.0

  @last = 0.0
  @busy = false
  @pending = nil
  @hooks = false
  @battle_id = nil
  @remote_id = nil
  @remote_name = nil
  @remote_appearance = nil
  @active = false
  @round_written = {}
  @hook_reg = false
  @forfeit_local = false
  @forfeit_remote = false
  @shared_seed = 0
  @battle_ref = nil
  @switch_seq = 0
  @remote_force_seq = 0
  @in_command_phase = false
  @last_chal_sent = 0.0
  @bag_snapshot = nil
  @held_snapshot = nil

  def self.log(m); end
  def self.active?; @active; end
  def self.in_command_phase?; @in_command_phase; end
  def self.busy?; @busy || @active; end

  def self.my_id
    begin; return FGL.instance_variable_get(:@my_id); rescue; return nil; end
  end

  def self.my_name
    begin
      n = FGL.player_display_name
      return n if n && n.to_s != ""
    rescue; end
    begin; return $Trainer.name.to_s; rescue; end
    "Player"
  end

  def self.ensure_dir
    begin; Dir.mkdir(DIR) unless File.directory?(DIR); rescue; end
  end

  def self.write_file(name, text)
    ensure_dir
    begin; File.open(File.join(DIR, name), "wb") { |f| f.write(text.to_s) }; rescue; end
  end

  def self.write_bin(name, data)
    ensure_dir
    begin; File.open(File.join(DIR, name), "wb") { |f| f.write(data) }; rescue; end
  end

  def self.read_file(name)
    p = File.join(DIR, name)
    return nil unless File.exist?(p)
    begin; return File.open(p, "rb") { |f| f.read }; rescue; return nil; end
  end

  def self.delete_file(name)
    p = File.join(DIR, name)
    begin; File.delete(p) if File.exist?(p); rescue; end
  end

  def self.snapshot_items
    @bag_snapshot = nil
    @held_snapshot = []
    begin
      if defined?($PokemonBag) && $PokemonBag
        @bag_snapshot = Marshal.load(Marshal.dump($PokemonBag))
      end
    rescue; end
    begin
      if $Trainer && $Trainer.party
        $Trainer.party.each_with_index do |pkmn, i|
          next if pkmn.nil?
          held = nil
          begin; held = pkmn.item if pkmn.respond_to?(:item); rescue; end
          @held_snapshot[i] = held
        end
      end
    rescue; end
  end

  def self.restore_items
    begin
      if @bag_snapshot && defined?($PokemonBag)
        $PokemonBag = @bag_snapshot
      end
    rescue; end
    begin
      if $Trainer && $Trainer.party && @held_snapshot
        $Trainer.party.each_with_index do |pkmn, i|
          next if pkmn.nil?
          begin
            pkmn.item = @held_snapshot[i] if pkmn.respond_to?(:item=)
          rescue; end
        end
      end
    rescue; end
    @bag_snapshot = nil
    @held_snapshot = nil
  end

  def self.force_save_play_slot
    begin
      slot = 0
      begin
        if defined?($PokemonGlobal) && $PokemonGlobal && $PokemonGlobal.respond_to?(:last_save_slot)
          slot = $PokemonGlobal.last_save_slot.to_i
        end
      rescue; end
      begin
        if defined?(Game) && Game.respond_to?(:save)
          Game.save(slot, true); return true
        end
      rescue; end
      begin
        if defined?(pbSave); pbSave(false); return true; end
      rescue; end
      begin
        if defined?(SaveData) && SaveData.respond_to?(:save_to_slot)
          SaveData.save_to_slot(slot); return true
        end
      rescue; end
    rescue; end
    false
  end

  def self.heal_party_silent
    party = nil
    begin; party = $Trainer.party; rescue; end
    return unless party
    party.each do |p|
      next if p.nil?
      begin; p.heal if p.respond_to?(:heal); rescue; end
    end
  end

  def self.save_party_bin(bid, id)
    heal_party_silent
    party = nil
    begin; party = $Trainer.party; rescue; end
    return false unless party
    begin
      write_bin("party_#{id}_#{bid}.bin", Marshal.dump(party))
      write_file("partyready_#{id}_#{bid}.txt", "1")
      return true
    rescue; return false; end
  end

  def self.load_party_bin(bid, oid)
    raw = read_file("party_#{oid}_#{bid}.bin")
    return nil if raw.nil? || raw.length < 2
    begin; return Marshal.load(raw); rescue; return nil; end
  end

  def self.hide_command_ui(battle)
    scene = nil
    begin; scene = battle.scene if battle.respond_to?(:scene); rescue; end
    return unless scene
    begin; scene.pbHideCommandMenu if scene.respond_to?(:pbHideCommandMenu); rescue; end
    begin; cw = scene.instance_variable_get(:@commandWindow); cw.visible = false if cw; rescue; end
    begin; fw = scene.instance_variable_get(:@fightWindow); fw.visible = false if fw; rescue; end
  end

  def self.force_msg_bottom!
    begin; $game_system.message_position = 2 if $game_system; rescue; end
  end

  def self.fix_message_bottom(battle = nil)
    force_msg_bottom!
    return unless battle
    scene = nil
    begin; scene = battle.scene if battle.respond_to?(:scene); rescue; end
    return unless scene
    ["@messageWindow", "@helpWindow", "@commandWindow"].each do |iv|
      begin
        w = scene.instance_variable_get(iv.to_sym)
        next unless w
        w.y = Graphics.height - w.height if w.respond_to?(:height)
      rescue; end
    end
  end

  def self.show_wait_msg(battle, text)
    hide_command_ui(battle)
    fix_message_bottom(battle)
    begin
      if battle.respond_to?(:pbDisplayBrief)
        battle.pbDisplayBrief(text)
        fix_message_bottom(battle)
        return
      end
    rescue; end
    begin
      battle.pbDisplay(text) if battle.respond_to?(:pbDisplay)
      fix_message_bottom(battle)
    rescue; end
  end

  def self.play_battle_music
    name = BATTLE_BGM.to_s
    begin
      if defined?($PokemonGlobal) && $PokemonGlobal
        $PokemonGlobal.nextBattleBGM = name
      end
    rescue; end
    begin; pbBGMPlay(name); rescue
      begin; Audio.bgm_play("Audio/BGM/#{name}", 100, 100); rescue; end
    end
  end

  def self.restore_map_music
    begin
      if defined?($PokemonGlobal) && $PokemonGlobal
        $PokemonGlobal.nextBattleBGM = nil
      end
    rescue; end
    begin
      if $game_map && $game_map.respond_to?(:autoplay)
        $game_map.autoplay; return
      end
    rescue; end
    begin; pbMapBGM if defined?(pbMapBGM); rescue; end
  end

  def self.clear_combat_bubble!
    begin
      if defined?($game_temp) && $game_temp
        $game_temp.in_battle = false
      end
    rescue; end
    begin
      mid = my_id
      if mid
        p = File.join(DIR, "#{mid}.txt")
        if File.exist?(p)
          line = File.open(p, "rb") { |f| f.read }.to_s
          parts = line.split("|")
          if parts.size > 15
            parts[15] = ""
            File.open(p, "wb") { |f| f.write(parts.join("|")) }
          end
        end
      end
    rescue; end
    begin; FGL.write_me if defined?(FGL) && FGL.respond_to?(:write_me); rescue; end
  end

  def self.cleanup_after_battle
    force_msg_bottom!
    begin
      if defined?($game_temp) && $game_temp
        $game_temp.message_window_showing = false rescue nil
        $game_temp.in_battle = false rescue nil
      end
    rescue; end
    clear_combat_bubble!
    3.times do
      Graphics.update rescue nil
      Input.update rescue nil
    end
  end

  def self.local_outfit_hash
    h = { :clothes => "", :hair => "", :hat => "", :hat2 => "",
          :clothes_color => 0, :hair_color => 0, :hat_color => 0,
          :hat2_color => 0, :skin_color => 0 }
    begin
      t = $Trainer
      h[:clothes] = t.clothes.to_s if t.respond_to?(:clothes)
      h[:hair] = t.hair.to_s if t.respond_to?(:hair)
      h[:hat] = t.hat.to_s if t.respond_to?(:hat)
      h[:hat2] = t.hat2.to_s if t.respond_to?(:hat2)
      h[:clothes_color] = t.clothes_color.to_i if t.respond_to?(:clothes_color)
      h[:hair_color] = t.hair_color.to_i if t.respond_to?(:hair_color)
      h[:hat_color] = t.hat_color.to_i if t.respond_to?(:hat_color)
      h[:hat2_color] = t.hat2_color.to_i if t.respond_to?(:hat2_color)
      h[:skin_color] = t.skin_tone.to_i if t.respond_to?(:skin_tone)
    rescue; end
    h
  end

  def self.outfit_to_str(h)
    [h[:clothes], h[:hair], h[:hat], h[:hat2],
     h[:clothes_color], h[:hair_color], h[:hat_color],
     h[:hat2_color], h[:skin_color]].join(",")
  end

  def self.str_to_outfit(s)
    a = s.to_s.split(",")
    { :clothes => (a[0] || ""), :hair => (a[1] || ""), :hat => (a[2] || ""),
      :hat2 => (a[3] || ""), :clothes_color => (a[4] || 0).to_i,
      :hair_color => (a[5] || 0).to_i, :hat_color => (a[6] || 0).to_i,
      :hat2_color => (a[7] || 0).to_i, :skin_color => (a[8] || 0).to_i }
  end

  def self.build_appearance(h)
    return nil if h.nil?
    begin; return FGLAppearance.new(h); rescue; return nil; end
  end

  def self.peer_outfit(id)
    players = nil
    begin; players = FGL.instance_variable_get(:@players); rescue; end
    return local_outfit_hash if players.nil? || !players[id]
    p = players[id]
    { :clothes => p[:clothes].to_s, :hair => p[:hair].to_s,
      :hat => p[:hat].to_s, :hat2 => p[:hat2].to_s,
      :clothes_color => (p[:cc] || p[:clothes_color] || 0).to_i,
      :hair_color => (p[:hc] || p[:hair_color] || 0).to_i,
      :hat_color => (p[:htc] || p[:hat_color] || 0).to_i,
      :hat2_color => (p[:h2c] || p[:hat2_color] || 0).to_i,
      :skin_color => (p[:skin] || p[:skin_color] || 0).to_i }
  end

  def self.find_peer_by_name(name)
    players = nil
    begin; players = FGL.instance_variable_get(:@players); rescue; end
    return nil unless players.is_a?(Hash)
    name_l = name.to_s.downcase.strip
    players.each do |id, data|
      pname = (data[:pname] || "").to_s.downcase.strip
      return [id, data[:pname].to_s, peer_outfit(id)] if pname == name_l
    end
    nil
  end

  def self.peer_is_busy?(peer_id)
    return true if peer_id.nil?
    ensure_dir
    begin
      Dir.glob(File.join(DIR, "chal_*")).each do |f|
        begin
          raw = File.read(f)
          if raw.include?(peer_id.to_s) || File.basename(f).include?(peer_id.to_s)
            age = 999
            begin; age = Time.now - File.mtime(f); rescue; end
            return true if age < 30.0
          end
        rescue; end
      end
    rescue; end
    false
  end

  def self.can_send_challenge?(target_id)
    now = 0.0
    begin; now = Time.now.to_f; rescue; end
    @last_chal_sent = 0.0 if @last_chal_sent.nil?
    if (now - @last_chal_sent) < CHAL_COOLDOWN
      return [false, _INTL("Please wait a moment before sending another challenge.")]
    end
    if busy? || active?
      return [false, _INTL("You are already in a battle or busy.")]
    end
    if peer_is_busy?(target_id)
      return [false, _INTL("This person is busy or already has a pending request.")]
    end
    [true, nil]
  end

  def self.poll_commands
    mid = my_id
    return if mid.nil?
    raw = read_file("cmd_#{mid}.txt")
    return if raw.nil? || raw.to_s.strip == ""
    line = raw.to_s.strip
    delete_file("cmd_#{mid}.txt")
    if line =~ /^\/battle\s+(.+)$/i
      challenge_player($1.strip)
    end
  end

  def self.challenge_player(target_name)
    return if busy?
    peer = find_peer_by_name(target_name)
    if peer.nil?
      force_msg_bottom!
      begin; pbMessage("\\wd" + _INTL("Player introuvable : {1}", target_name)); rescue; end
      return
    end
    tid, tname, _o = peer
    mid = my_id
    return if mid.nil?
    ok, msg = can_send_challenge?(tid)
    unless ok
      force_msg_bottom!
      begin; pbMessage("\\wd" + msg.to_s); rescue; end
      return
    end
    bid = "#{Time.now.to_i}"
    seed = rand(999999999)
    my_out = outfit_to_str(local_outfit_hash)
    @pending = { :role => "challenger", :target => tid, :name => tname, :bid => bid, :seed => seed }
    begin; @last_chal_sent = Time.now.to_f; rescue; @last_chal_sent = 0.0; end
    write_file("chal_#{tid}.txt", ["CHALLENGE", mid, tid, my_name, bid, seed, my_out].join("|"))
    force_msg_bottom!
    pbMessage("\\wd" + _INTL("Battle request sent to {1}.", tname)) rescue nil
  end

  def self.poll_incoming
    mid = my_id
    return if mid.nil?
    raw = read_file("chal_#{mid}.txt")
    return if raw.nil? || raw.to_s.strip == ""
    parts = raw.to_s.strip.split("|")
    delete_file("chal_#{mid}.txt")
    return unless parts[0] == "CHALLENGE"
    from = parts[1].to_s
    from_name = parts[3].to_s
    bid = parts[4].to_s
    seed = parts[5].to_i
    from_outfit = str_to_outfit(parts[6] || "")
    if busy?
      write_file("chalresp_#{from}.txt", ["DECLINE", mid, from, bid, "busy"].join("|"))
      return
    end
    force_msg_bottom!
    ok = false
    begin; ok = pbConfirmMessage("\\wd" + _INTL("{1} challenges you to a battle! Accept?", from_name)); rescue; ok = false; end
    if ok
      save_party_bin(bid, mid)
      my_out = outfit_to_str(local_outfit_hash)
      write_file("chalresp_#{from}.txt", ["ACCEPT", mid, from, bid, my_name, seed, my_out].join("|"))
      @busy = true
      @remote_appearance = from_outfit
      @remote_name = from_name
      @shared_seed = seed
      wait_and_start(bid, from, from_name, seed)
    else
      write_file("chalresp_#{from}.txt", ["DECLINE", mid, from, bid, "no"].join("|"))
    end
  end

  def self.poll_response
    return unless @pending && @pending[:role] == "challenger"
    raw = read_file("chalresp_#{my_id}.txt")
    return if raw.nil? || raw.to_s.strip == ""
    parts = raw.to_s.strip.split("|")
    delete_file("chalresp_#{my_id}.txt")
    if parts[0] == "DECLINE"
      force_msg_bottom!
      begin; pbMessage("\\wd" + _INTL("Challenge declined by {1}.", @pending[:name].to_s)); rescue; end
      @pending = nil
      return
    end
    if parts[0] == "ACCEPT"
      bid = @pending[:bid]
      seed = @pending[:seed].to_i
      other = @pending[:target]
      name = @pending[:name]
      @remote_appearance = str_to_outfit(parts[6] || "")
      @remote_name = name
      @shared_seed = seed
      save_party_bin(bid, my_id)
      @busy = true
      @pending = nil
      wait_and_start(bid, other, name, seed)
    end
  end

  def self.wait_and_start(bid, other_id, other_name, seed)
    @remote_name = other_name if other_name.to_s != ""
    @shared_seed = seed
    force_msg_bottom!
    begin; pbMessage("\\wd" + _INTL("Preparing the battle...")); rescue; end
    heal_party_silent
    snapshot_items
    force_save_play_slot
    12.times do
      Graphics.update rescue nil
      Input.update rescue nil
    end
    foe = nil
    180.times do
      Graphics.update rescue nil
      Input.update rescue nil
      p = load_party_bin(bid, other_id)
      if p && p.length > 0
        foe = p
        break
      end
    end
    if foe.nil? || foe.length == 0
      @busy = false
      cleanup_after_battle
      force_msg_bottom!
      begin; pbMessage("\\wd" + _INTL("Unable to start the battle.")); rescue; end
      return
    end
    @battle_id = bid
    @remote_id = other_id
    @round_written = {}
    @forfeit_local = false
    @forfeit_remote = false
    @switch_seq = 0
    @remote_force_seq = 0
    @in_command_phase = false
    delete_file("bat_#{bid}_#{my_id}_fsw_last.txt")
    delete_file("bat_#{bid}_#{other_id}_fsw_last.txt")
    2.times do
      Graphics.update rescue nil
      Input.update rescue nil
    end
    start_human_vs_human(foe, other_name, seed)
  end

  def self.action_name(pid, round)
    "bat_#{@battle_id}_#{pid}_r#{round}.txt"
  end

  def self.parse_force_seq(raw)
    return 0 if raw.nil?
    parts = raw.to_s.strip.split("|")
    return 0 if parts.length < 3
    parts[2].to_i
  end

  def self.parse_force_idx(raw)
    return -1 if raw.nil?
    parts = raw.to_s.strip.split("|")
    return -1 if parts.length < 2
    parts[1].to_i
  end

  def self.safe_turn_count(battle)
    tc = 0
    begin; tc = battle.turnCount.to_i; rescue; tc = 0; end
    tc
  end

  def self.publish_local_move(idxMove, battle)
    return unless @active
    round = safe_turn_count(battle)
    key = "r#{round}_move"
    return if @round_written[key]
    @round_written[key] = true
    write_file(action_name(my_id, round), "MOVE|#{idxMove}")
  end

  def self.publish_local_switch(idxParty, battle)
    return unless @active
    round = safe_turn_count(battle)
    key = "r#{round}_switch"
    return if @round_written[key]
    @round_written[key] = true
    write_file(action_name(my_id, round), "SWITCH|#{idxParty}")
  end

  def self.publish_local_force_switch(idxParty, battle = nil)
    return unless @active
    return if idxParty.nil?
    idx = idxParty.to_i
    return if idx < 0
    @switch_seq = (@switch_seq.to_i + 1)
    seq = @switch_seq
    write_file("bat_#{@battle_id}_#{my_id}_fsw_last.txt", "FORCE_SWITCH|#{idx}|#{seq}")
  end

  def self.publish_local_run(battle)
    return unless @active
    round = safe_turn_count(battle)
    key = "r#{round}_run"
    return if @round_written[key]
    @round_written[key] = true
    @forfeit_local = true
    write_file(action_name(my_id, round), "RUN")
  end

  def self.wait_remote_human(battle)
    round = safe_turn_count(battle)
    fname = action_name(@remote_id, round)
    rname = "Trainer"
    rname = @remote_name if @remote_name && @remote_name.to_s != ""
    hide_command_ui(battle)
    shown = false
    20000.times do
      t = read_file(fname)
      if t && t.to_s.strip != ""
        return t.to_s.strip
      end
      if !shown
        shown = true
        show_wait_msg(battle, _INTL("Waiting for an action from {1}...", rname))
      else
        hide_command_ui(battle)
        fix_message_bottom(battle)
      end
      Graphics.update rescue nil
      Input.update rescue nil
    end
    nil
  end

  def self.wait_remote_force_switch(battle)
    rname = "Trainer"
    rname = @remote_name if @remote_name && @remote_name.to_s != ""
    fname = "bat_#{@battle_id}_#{@remote_id}_fsw_last.txt"
    existing = read_file(fname)
    disk_seq = parse_force_seq(existing)
    min_seq = @remote_force_seq.to_i
    min_seq = disk_seq if disk_seq > min_seq
    hide_command_ui(battle)
    shown = false
    30000.times do
      raw = read_file(fname)
      if raw && raw.to_s.strip != ""
        seq = parse_force_seq(raw)
        if seq > min_seq
          @remote_force_seq = seq
          return raw.to_s.strip
        end
      end
      if !shown
        shown = true
        show_wait_msg(battle, _INTL("{1} is choosing a Pokémon...", rname))
      else
        hide_command_ui(battle)
        fix_message_bottom(battle)
      end
      Graphics.update rescue nil
      Input.update rescue nil
    end
    nil
  end

  def self.pick_ttype
    [:POKEMONTRAINER_Leaf, :RIVAL00, :RIVAL, :BLUE, :YOUNGSTER].each do |t|
      begin
        if defined?(GameData::TrainerType) && GameData::TrainerType.exists?(t)
          return t
        end
      rescue; end
    end
    :YOUNGSTER
  end

  def self.force_seed(extra = 0)
    s = (@shared_seed.to_i + extra.to_i)
    begin; srand(s); rescue; end
    begin; Kernel.srand(s) if Kernel.respond_to?(:srand); rescue; end
  end

  def self.attach_human_controller(battle)
    ctrl = FGLHumanRemote.new(battle)
    begin; battle.instance_variable_set(:@battleAI, ctrl); rescue; end
    begin; battle.instance_variable_set(:@AI, ctrl); rescue; end
    begin
      battle.battleAI = ctrl if battle.respond_to?(:battleAI=)
    rescue; end
    @battle_ref = battle
    ctrl
  end

  def self.disable_rewards(battle)
    begin; battle.expGain = false if battle.respond_to?(:expGain=); rescue; end
    begin; battle.moneyGain = false if battle.respond_to?(:moneyGain=); rescue; end
    begin; battle.internalBattle = true; rescue; end
    begin; battle.noMoney = true if battle.respond_to?(:noMoney=); rescue; end
  end

  def self.start_human_vs_human(foe_party, foe_name, seed)
    foe_name = "Player" if foe_name.to_s == ""
    @remote_name = foe_name if @remote_name.to_s == ""
    @shared_seed = seed
    @active = true
    force_seed(0)
    decision = 0
    begin
      heal_party_silent
      scene = nil
      begin; scene = pbNewBattleScene; rescue; scene = PokeBattle_Scene.new; end
      appearance = build_appearance(@remote_appearance)
      shell = nil
      begin
        shell = NPCTrainer.new(foe_name, pick_ttype, nil, appearance)
      rescue
        begin; shell = NPCTrainer.new(foe_name, pick_ttype); rescue
          shell = NPCTrainer.new(foe_name, :YOUNGSTER)
        end
      end
      shell.party = foe_party
      begin
        shell.custom_appearance = appearance if appearance && shell.respond_to?(:custom_appearance=)
      rescue; end
      begin
        shell.instance_variable_set(:@custom_appearance, appearance) if appearance
      rescue; end
      battle = PokeBattle_Battle.new(scene, $Trainer.party, foe_party, [$Trainer], [shell])
      disable_rewards(battle)
      begin; battle.canRun = true; rescue; end
      attach_human_controller(battle)
      install_hooks_once
      begin; pbPrepareBattle(battle); rescue; end
      disable_rewards(battle)
      attach_human_controller(battle)
      force_seed(1)
      play_battle_music
      decision = battle.pbStartBattle
      cleanup_after_battle
      force_msg_bottom!
      show_end_messages(decision, foe_name)
      cleanup_after_battle
    rescue Exception => e
      cleanup_after_battle
      force_msg_bottom!
      begin; pbMessage("\\wd" + _INTL("Battle error.")); rescue; end
    ensure
      heal_party_silent
      restore_items
      cleanup_after_battle
      restore_map_music
      @active = false
      @busy = false
      @battle_id = nil
      @remote_id = nil
      @remote_name = nil
      @remote_appearance = nil
      @round_written = {}
      @forfeit_local = false
      @forfeit_remote = false
      @battle_ref = nil
      @switch_seq = 0
      @remote_force_seq = 0
      @in_command_phase = false
    end
  end

  def self.show_end_messages(decision, foe_name)
    name = "the trainer"
    name = foe_name if foe_name && foe_name.to_s != ""
    force_msg_bottom!
    begin
      if @forfeit_local
        pbMessage("\\wd" + _INTL("You have forfeited.")) rescue nil
        pbMessage("\\wd" + _INTL("You lost against {1}.", name)) rescue nil
      elsif @forfeit_remote
        pbMessage("\\wd" + _INTL("{1} has forfeited.", name)) rescue nil
        pbMessage("\\wd" + _INTL("You won against {1}.", name)) rescue nil
      elsif decision == 1
        pbMessage("\\wd" + _INTL("You won against {1}.", name)) rescue nil
      elsif decision == 2
        pbMessage("\\wd" + _INTL("You lost against {1}.", name)) rescue nil
      else
        pbMessage("\\wd" + _INTL("End of the battle.")) rescue nil
      end
    rescue; end
  end

  def self.install_hooks_once
    return unless defined?(PokeBattle_Battle)
    return if @hook_reg
    @hook_reg = true
    PokeBattle_Battle.class_eval do
      unless method_defined?(:_fgl_pbRegisterMove)
        alias_method :_fgl_pbRegisterMove, :pbRegisterMove
        def pbRegisterMove(idxBattler, idxMove, *args)
          begin
            if FGLBattle.active? && pbOwnedByPlayer?(idxBattler)
              FGLBattle.publish_local_move(idxMove, self)
            end
          rescue; end
          _fgl_pbRegisterMove(idxBattler, idxMove, *args)
        end
      end
      unless method_defined?(:_fgl_pbRun)
        alias_method :_fgl_pbRun, :pbRun
        def pbRun(idxBattler, *args)
          begin
            if FGLBattle.active? && pbOwnedByPlayer?(idxBattler)
              FGLBattle.publish_local_run(self)
              @decision = 2
              return 1
            end
          rescue; end
          _fgl_pbRun(idxBattler, *args)
        end
      end
      unless method_defined?(:_fgl_pbRegisterSwitch)
        if method_defined?(:pbRegisterSwitch)
          alias_method :_fgl_pbRegisterSwitch, :pbRegisterSwitch
          def pbRegisterSwitch(idxBattler, idxParty, *args)
            begin
              if FGLBattle.active? && pbOwnedByPlayer?(idxBattler)
                if FGLBattle.in_command_phase?
                  FGLBattle.publish_local_switch(idxParty, self)
                else
                  FGLBattle.publish_local_force_switch(idxParty, self)
                end
              end
            rescue; end
            _fgl_pbRegisterSwitch(idxBattler, idxParty, *args)
          end
        end
      end
      unless method_defined?(:_fgl_pbSwitch)
        if method_defined?(:pbSwitch)
          alias_method :_fgl_pbSwitch, :pbSwitch
          def pbSwitch(*args)
            ret = _fgl_pbSwitch(*args)
            begin
              if FGLBattle.active? && ret.is_a?(Integer) && ret >= 0
                unless FGLBattle.in_command_phase?
                  FGLBattle.publish_local_force_switch(ret, self)
                end
              end
            rescue; end
            ret
          end
        end
      end
      unless method_defined?(:_fgl_pbSwitchInBetween)
        if method_defined?(:pbSwitchInBetween)
          alias_method :_fgl_pbSwitchInBetween, :pbSwitchInBetween
          def pbSwitchInBetween(*args)
            ret = nil
            if block_given?
              ret = _fgl_pbSwitchInBetween(*args) { |*ya| yield(*ya) }
            else
              ret = _fgl_pbSwitchInBetween(*args)
            end
            begin
              if FGLBattle.active? && ret.is_a?(Integer) && ret >= 0
                idxBattler = args[0]
                owned = true
                begin; owned = pbOwnedByPlayer?(idxBattler); rescue; end
                if owned && !FGLBattle.in_command_phase?
                  FGLBattle.publish_local_force_switch(ret, self)
                end
              end
            rescue; end
            ret
          end
        end
      end
      unless method_defined?(:_fgl_pbCommandPhase)
        alias_method :_fgl_pbCommandPhase, :pbCommandPhase
        def pbCommandPhase
          if FGLBattle.active?
            FGLBattle.instance_variable_set(:@in_command_phase, true)
            FGLBattle.attach_human_controller(self)
            FGLBattle.disable_rewards(self)
            tc = 0
            begin; tc = self.turnCount.to_i; rescue; tc = 0; end
            FGLBattle.force_seed(tc)
          end
          begin
            _fgl_pbCommandPhase
          ensure
            if FGLBattle.active?
              FGLBattle.instance_variable_set(:@in_command_phase, false)
            end
          end
        end
      end
      unless method_defined?(:_fgl_pbAttackPhase)
        if method_defined?(:pbAttackPhase)
          alias_method :_fgl_pbAttackPhase, :pbAttackPhase
          def pbAttackPhase
            if FGLBattle.active?
              FGLBattle.instance_variable_set(:@in_command_phase, false)
              tc = 0
              begin; tc = self.turnCount.to_i; rescue; tc = 0; end
              FGLBattle.force_seed(tc + 1000)
            end
            _fgl_pbAttackPhase
          end
        end
      end
    end
    if defined?(PokeBattle_Scene)
      PokeBattle_Scene.class_eval do
        unless method_defined?(:_fgl_pbShowOpponent)
          if method_defined?(:pbShowOpponent)
            alias_method :_fgl_pbShowOpponent, :pbShowOpponent
            def pbShowOpponent(*args)
              return if FGLBattle.active?
              _fgl_pbShowOpponent(*args)
            end
          end
        end
        unless method_defined?(:_fgl_pbTrainerBattleSpeech)
          if method_defined?(:pbTrainerBattleSpeech)
            alias_method :_fgl_pbTrainerBattleSpeech, :pbTrainerBattleSpeech
            def pbTrainerBattleSpeech(*args)
              return if FGLBattle.active?
              _fgl_pbTrainerBattleSpeech(*args)
            end
          end
        end
      end
    end
  rescue; end

  def self.tick
    now = 0.0
    begin; now = Time.now.to_f; rescue; end
    return if now - @last < TICK
    @last = now
    return if @active
    begin; return unless $scene.is_a?(Scene_Map); rescue; return; end
    begin; poll_commands; rescue; end
    begin; poll_incoming; rescue; end
    begin; poll_response; rescue; end
  end

  def self.install!
    return if @hooks
    @hooks = true
    meta = (class << Graphics; self; end)
    meta.class_eval do
      unless method_defined?(:_fglb_u)
        alias_method :_fglb_u, :update
        def update
          _fglb_u
          begin; FGLBattle.tick; rescue; end
        end
      end
    end
  end
end

class FGLAppearance
  attr_accessor :clothes, :hat, :hat2, :hair, :skin_color
  attr_accessor :hair_color, :hat_color, :clothes_color, :hat2_color
  def initialize(h)
    h = {} if h.nil?
    @clothes = h[:clothes]
    @hat = h[:hat]
    @hat2 = h[:hat2]
    @hair = h[:hair]
    @skin_color = h[:skin_color]
    @hair_color = h[:hair_color]
    @hat_color = h[:hat_color]
    @clothes_color = h[:clothes_color]
    @hat2_color = h[:hat2_color]
  end
end

class FGLHumanRemote
  def initialize(battle)
    @battle = battle
  end
  def pbAIRandom(x); rand(x); end
  def pbDefaultChooseEnemyCommand(idxBattler)
    raw = FGLBattle.wait_remote_human(@battle)
    if raw.nil?
      FGLBattle.instance_variable_set(:@forfeit_remote, true)
      @battle.instance_variable_set(:@decision, 1)
      return
    end
    parts = raw.split("|")
    if parts[0] == "MOVE"
      @battle.pbRegisterMove(idxBattler, parts[1].to_i, false)
    elsif parts[0] == "SWITCH"
      idx = parts[1].to_i
      begin
        @battle.pbRegisterSwitch(idxBattler, idx) if @battle.respond_to?(:pbRegisterSwitch)
      rescue
        begin; @battle.pbRegisterMove(idxBattler, 0, false); rescue; end
      end
    elsif parts[0] == "RUN"
      FGLBattle.instance_variable_set(:@forfeit_remote, true)
      @battle.instance_variable_set(:@decision, 1)
    else
      begin; @battle.pbRegisterMove(idxBattler, 0, false); rescue; end
    end
  end
  def pbChooseMoves(idxBattler); pbDefaultChooseEnemyCommand(idxBattler); end
  def pbEnemyShouldWithdraw?(idxBattler); false; end
  def pbEnemyShouldUseItem?(idxBattler); false; end
  def pbEnemyShouldMegaEvolve?(idxBattler); false; end
  def pbDefaultChooseNewEnemy(idxBattler, party)
    raw = FGLBattle.wait_remote_force_switch(@battle)
    if raw && raw.to_s.strip != ""
      idx = FGLBattle.parse_force_idx(raw)
      begin
        if idx >= 0 && party && party[idx]
          fainted = true
          begin; fainted = party[idx].fainted?; rescue; end
          return idx unless fainted
        end
      rescue; end
    end
    return -1 if party.nil?
    party.each_with_index do |p, i|
      next if p.nil?
      begin
        if !p.fainted?
          return i if !p.respond_to?(:egg?) || !p.egg?
        end
      rescue
        return i
      end
    end
    -1
  end
end

begin
  FGLBattle.install!
rescue
end