# =============================================================================
# FGL_RemotePlayer_Test.rb — PROTOTYPE EXPERIMENTAL (isole)
# =============================================================================
# Joueur distant = Game_Character + Sprite_Character via IPC launcher UNIQUEMENT.
# Chemin: Game -> IPC (FGL_IPC_PORT) -> Launcher -> EasyTier -> Launcher -> IPC -> Game
# - Ne modifie PAS FGL_Net / Trade / Battle
# - Transport IPC launcher uniquement (pas de fichiers peers, pas de transport parallele)
# - map_id distant N'IMPOSE PAS la map locale
# =============================================================================

module FGL_RemotePlayer_Test
  TICK = 0.05
  STALE_MISS = 120
  STATUS_EVERY = 30

  @sock = nil
  @my_id = nil
  @last = 0.0
  @remotes = {}
  @hooks_done = false
  @status_n = 0
  @last_err = ""
  @poll_raw = 0
  @ipc_inbox = []
  @ipc_fanout_done = false

  def self.status_path
    begin
      desk = ENV["USERPROFILE"].to_s
      return File.join(desk, "Desktop", "fgl_rpt_status.txt") if desk != ""
    rescue
    end
    begin
      return File.join(ENV["TEMP"].to_s, "fgl_rpt_status.txt")
    rescue
    end
    "fgl_rpt_status.txt"
  end

  def self.write_status(extra = "")
    begin
      lines = []
      lines << "time=#{Time.now}"
      lines << "ipc_port=#{ipc_port}"
      lines << "my_id=#{@my_id}"
      lines << "poll_raw=#{@poll_raw.to_i}"
      lines << "inbox=#{(@ipc_inbox ? @ipc_inbox.size : 0)}"
      lines << "remotes=#{@remotes.size}"
      lines << "scene=#{$scene ? $scene.class.name : "nil"}"
      lines << "vp=#{map_viewport ? "ok" : "nil"}"
      lines << "err=#{@last_err}"
      @remotes.each do |id, rec|
        d = rec[:data] || {}
        lines << "remote id=#{id} map=#{d[:map]} x=#{d[:x]} y=#{d[:y]} dir=#{d[:dir]} cname=#{d[:cname]} spr=#{rec[:sprite] ? "yes" : "no"}"
      end
      lines << extra if extra.to_s != ""
      File.open(status_path, "w") { |f| f.puts lines.join("\n") }
    rescue
    end
  end

  def self.ipc_port
    p = ENV["FGL_IPC_PORT"].to_s.to_i
    return 0 if p < 1 || p > 65535
    p
  end

  def self.ensure_sock
    nil
  end

  def self.install_ipc_fanout!
    return if @ipc_fanout_done
    @ipc_fanout_done = true
    @ipc_inbox = [] unless @ipc_inbox
    begin
      return unless defined?(FGL_IPC) && FGL_IPC.respond_to?(:poll)
      return if FGL_IPC.respond_to?(:_fgl_rpt_poll_orig)
      FGL_IPC.singleton_class.class_eval do
        alias_method :_fgl_rpt_poll_orig, :poll
        def poll
          batch = _fgl_rpt_poll_orig
          begin
            if batch.is_a?(Array) && batch.size > 0
              FGL_RemotePlayer_Test.push_ipc_batch(batch)
            end
          rescue
          end
          batch
        end
      end
    rescue => e
      @last_err = "fanout:#{e}"
      @ipc_fanout_done = false
    end
  end

  def self.push_ipc_batch(batch)
    @ipc_inbox = [] unless @ipc_inbox
    batch.each { |line| @ipc_inbox << line.to_s }
    while @ipc_inbox.size > 256
      @ipc_inbox.shift
    end
  end

  def self.ipc_send(line)
    p = ipc_port
    return if p <= 0
    begin
      if defined?(FGL_IPC) && FGL_IPC.respond_to?(:send_player_line)
        FGL_IPC.send_player_line(line)
        return
      end
    rescue => e
      @last_err = "send:#{e}"
    end
  end

  def self.ipc_poll
    install_ipc_fanout!
    return [] if ipc_port <= 0
    @ipc_inbox = [] unless @ipc_inbox
    out = []
    while @ipc_inbox.size > 0 && out.size < 32
      out << @ipc_inbox.shift
    end
    out
  end

  def self.ensure_id
    return if @my_id
    @my_id = "#{Time.now.to_i}_#{rand(999999)}"
  end

  def self.safe_text(s)
    s.to_s.gsub("|", "").gsub("\n", " ").strip[0, 32]
  end

  def self.clean_name(path)
    return "walk" if path.nil? || path.to_s.empty?
    s = path.to_s.gsub("\\", "/")
    s = s.sub(%r{^Graphics/Characters/}i, "")
    s = s.sub(%r{^/+}, "")
    s = s.sub(/\.(png|jpg|bmp)$/i, "")
    s = s.gsub("|", "")
    s.empty? ? "walk" : s
  end

  def self.map_viewport
    begin
      return Spriteset_Map.viewport if defined?(Spriteset_Map) && Spriteset_Map.respond_to?(:viewport)
    rescue
    end
    return nil unless $scene
    s = nil
    begin
      s = $scene.spriteset if $scene.respond_to?(:spriteset)
    rescue
    end
    if s.nil?
      begin
        s = $scene.instance_variable_get(:@spriteset)
      rescue
      end
    end
    return nil unless s
    begin
      return s.viewport1 if s.respond_to?(:viewport1)
    rescue
    end
    begin
      v = s.instance_variable_get(:@viewport1)
      return v if v
    rescue
    end
    begin
      v = s.instance_variable_get(:@viewport)
      return v if v
    rescue
    end
    nil
  end

  def self.player_display_name
    begin
      return safe_text($player.name) if defined?($player) && $player && $player.name
    rescue
    end
    begin
      return safe_text($Trainer.name) if defined?($Trainer) && $Trainer && $Trainer.name
    rescue
    end
    "Player"
  end

  def self.detect_move_state
    begin
      return 1 if defined?($PokemonGlobal) && $PokemonGlobal && $PokemonGlobal.surfing
      return 4 if defined?($PokemonGlobal) && $PokemonGlobal && $PokemonGlobal.diving
      return 2 if defined?($PokemonGlobal) && $PokemonGlobal && $PokemonGlobal.bicycle
      return 3 if defined?($PokemonGlobal) && $PokemonGlobal && $PokemonGlobal.fishing
    rescue
    end
    begin
      cn = $game_player.character_name.to_s.downcase
      return 1 if cn.include?("surf")
      return 4 if cn.include?("dive")
      return 2 if cn.include?("bike")
      return 3 if cn.include?("fish")
    rescue
    end
    0
  end

  def self.read_trainer_outfit
    clothes = hair = hat = hat2 = ""
    cc = hc = htc = h2c = skin = 0
    surfmon = ""
    bike_col = 0
    begin
      if defined?($Trainer) && $Trainer
        t = $Trainer
        clothes = t.clothes.to_s if t.respond_to?(:clothes)
        hair    = t.hair.to_s if t.respond_to?(:hair)
        hat     = t.hat.to_s if t.respond_to?(:hat)
        hat2    = t.hat2.to_s if t.respond_to?(:hat2)
        cc  = t.clothes_color.to_i if t.respond_to?(:clothes_color)
        hc  = t.hair_color.to_i if t.respond_to?(:hair_color)
        htc = t.hat_color.to_i if t.respond_to?(:hat_color)
        h2c = t.hat2_color.to_i if t.respond_to?(:hat2_color)
        skin = t.skin_tone.to_i if t.respond_to?(:skin_tone)
        bike_col = t.bike_color.to_i if t.respond_to?(:bike_color)
        if t.respond_to?(:surfing_pokemon) && t.surfing_pokemon
          begin
            sp = t.surfing_pokemon
            surfmon = sp.respond_to?(:species) ? sp.species.to_s : sp.to_s
          rescue
            surfmon = t.surfing_pokemon.to_s
          end
        end
      end
    rescue
    end
    [safe_text(clothes), safe_text(hair), safe_text(hat), safe_text(hat2),
     cc, hc, htc, h2c, skin, safe_text(surfmon), bike_col]
  end

  def self.write_local
    return unless $game_player && $game_map
    return if ipc_port <= 0
    ensure_id
    p = $game_player
    cn = nil
    begin
      cn = p.character_name
    rescue
      cn = nil
    end
    cname = clean_name(cn || "walk")
    cname = "walk" if cname.empty?
    px = p.x.to_i rescue 0
    py = p.y.to_i rescue 0
    pdir = p.direction.to_i rescue 2
    pspeed = p.move_speed.to_i rescue 3
    ppat = p.pattern.to_i rescue 0
    begin
      ppat = p.pattern_surf.to_i if detect_move_state == 1 && p.respond_to?(:pattern_surf)
    rescue
    end
    mid = ($game_map.map_id rescue 0).to_i
    pname = player_display_name
    action = ""
    state = detect_move_state
    clothes, hair, hat, hat2, cc, hc, htc, h2c, skin, surfmon, bike_col = read_trainer_outfit
    line = [
      @my_id, "P", mid, px, py, pdir, cname, pspeed, ppat, 0,
      0, 0, 0, 0, 0, action, pname,
      clothes, hair, hat, hat2, cc, hc, htc, h2c, skin, state, surfmon, bike_col, Time.now.to_i
    ].join("|")
    ipc_send(line)
  end

  def self.parse_player_raw(raw)
    s = raw.to_s
    s = s[7, s.length - 7] if s.index("PLAYER|") == 0
    a = s.strip.split("|")
    return nil if a.size < 7
    id = a[0].to_s
    return nil if id.empty?
    return nil if @my_id && id == @my_id
    return nil if a[1].to_s != "P" && a[1].to_s != ""
    {
      :id => id,
      :map => a[2].to_i,
      :x => a[3].to_i,
      :y => a[4].to_i,
      :dir => a[5].to_i,
      :cname => clean_name(a[6]),
      :speed => a[7].to_i,
      :pattern => a[8].to_i,
      :action => (a.size > 15 ? safe_text(a[15]) : ""),
      :pname => (a.size > 16 ? safe_text(a[16]) : "Player"),
      :clothes => (a.size > 17 ? safe_text(a[17]) : ""),
      :hair => (a.size > 18 ? safe_text(a[18]) : ""),
      :hat => (a.size > 19 ? safe_text(a[19]) : ""),
      :hat2 => (a.size > 20 ? safe_text(a[20]) : ""),
      :cc => (a.size > 21 ? a[21].to_i : 0),
      :hc => (a.size > 22 ? a[22].to_i : 0),
      :htc => (a.size > 23 ? a[23].to_i : 0),
      :h2c => (a.size > 24 ? a[24].to_i : 0),
      :skin => (a.size > 25 ? a[25].to_i : 0),
      :state => (a.size > 26 ? a[26].to_i : 0),
      :surfmon => (a.size > 27 ? safe_text(a[27]) : ""),
      :bike_col => (a.size > 28 ? a[28].to_i : 0)
    }
  end

  def self.ensure_sprite(rec)
    return if rec[:sprite] && !(rec[:sprite].disposed? rescue true)
    v = map_viewport
    return unless v
    return unless defined?(Sprite_Character)
    begin
      rec[:sprite] = Sprite_Character.new(v, rec[:char])
      begin
        rec[:sprite].visible = true
      rescue
      end
    rescue => e
      @last_err = "sprite:#{e}"
      rec[:sprite] = nil
    end
  end

  def self.ingest_network
    ensure_id
    return if ipc_port <= 0
    seen = {}
    batch = ipc_poll
    @poll_raw = (@poll_raw || 0) + batch.size
    batch.each do |raw|
      data = parse_player_raw(raw)
      next unless data
      id = data[:id]
      seen[id] = true
      rec = @remotes[id]
      if rec.nil?
        char = FGL_RemoteCharacter.new
        char.apply_net(data)
        rec = { :char => char, :sprite => nil, :miss => 0, :data => data }
        @remotes[id] = rec
        ensure_sprite(rec)
      else
        rec[:char].apply_net(data)
        rec[:data] = data
        rec[:miss] = 0
        ensure_sprite(rec)
      end
    end
    @remotes.keys.each do |id|
      next if seen[id]
      @remotes[id][:miss] = (@remotes[id][:miss] || 0) + 1
      kill_remote(id) if @remotes[id][:miss] > STALE_MISS
    end
  end

  def self.kill_remote(id)
    rec = @remotes[id]
    return unless rec
    begin
      if rec[:sprite]
        rec[:sprite].dispose unless (rec[:sprite].disposed? rescue false)
      end
    rescue
    end
    @remotes.delete(id)
  end

  def self.update_sprites
    @remotes.each_value do |rec|
      begin
        rec[:char].update if rec[:char].respond_to?(:update)
      rescue
      end
      ensure_sprite(rec)
      begin
        if rec[:sprite] && !(rec[:sprite].disposed? rescue true)
          rec[:sprite].update
          rec[:sprite].visible = true
        end
      rescue => e
        @last_err = "upd:#{e}"
      end
    end
  end

  def self.clear_all
    @remotes.keys.each { |id| kill_remote(id) }
    @remotes.clear
  end

  def self.tick
    return unless $game_player && $game_map
    now = Time.now.to_f
    begin
      now = System.uptime
    rescue
    end
    return if now - @last < TICK
    @last = now
    begin
      write_local
    rescue => e
      @last_err = "write:#{e}"
    end
    begin
      return unless $scene.is_a?(Scene_Map)
    rescue
      return
    end
    begin
      ingest_network
    rescue => e
      @last_err = "ingest:#{e}"
    end
    begin
      update_sprites
    rescue => e
      @last_err = "sprites:#{e}"
    end
    @status_n += 1
    if @status_n == 1 || (@status_n % STATUS_EVERY) == 0
      write_status("tick=#{@status_n}")
    end
  end

  def self.install_hooks!
    return if @hooks_done
    install_ipc_fanout!
    ok = false
    begin
      if defined?(Graphics)
        meta = (class << Graphics; self; end)
        meta.class_eval do
          unless method_defined?(:_fgl_rpt_graphics_update)
            alias_method :_fgl_rpt_graphics_update, :update
            def update
              _fgl_rpt_graphics_update
              begin
                FGL_RemotePlayer_Test.tick
              rescue
              end
            end
          end
        end
        ok = true
      end
    rescue => e
      @last_err = "hookG:#{e}"
    end
    begin
      if defined?(Scene_Map) && Scene_Map.method_defined?(:update)
        Scene_Map.class_eval do
          unless method_defined?(:_fgl_rpt_scene_update)
            alias_method :_fgl_rpt_scene_update, :update
            def update
              _fgl_rpt_scene_update
              begin
                FGL_RemotePlayer_Test.update_sprites
              rescue
              end
            end
          end
        end
        ok = true
      end
    rescue => e
      @last_err = "hookS:#{e}"
    end
    begin
      if defined?(EventHandlers)
        EventHandlers.add(:on_new_spriteset_map, :fgl_remote_player_test_clear, proc {
          begin
            FGL_RemotePlayer_Test.clear_all
          rescue
          end
        })
      end
    rescue
    end
    @hooks_done = ok
    write_status("hooks=#{ok}")
  end
end

class FGL_RemoteCharacter < Game_Character
  attr_reader :net_id, :net_map_id, :net_pname
  attr_reader :net_clothes, :net_hair, :net_hat, :net_hat2, :net_state

  def initialize
    begin
      super()
    rescue ArgumentError
      begin
        super($game_map)
      rescue
        @x = 0
        @y = 0
        @real_x = 0
        @real_y = 0
        @direction = 2
        @pattern = 0
        @move_speed = 3
        @character_name = "walk"
        @character_hue = 0
        @opacity = 255
        @blend_type = 0
        @tile_id = 0
      end
    end
    @net_id = nil
    @net_map_id = 0
    @net_pname = "Player"
    @net_clothes = ""
    @net_hair = ""
    @net_hat = ""
    @net_hat2 = ""
    @net_state = 0
    @through = true
    @move_frequency = 6
    @walk_anime = true
    @step_anime = false
    @direction_fix = false
    @opacity = 255
    @locked_pattern = 0
  end

  def update
    begin
      if respond_to?(:update_animation, true)
        update_animation
      end
    rescue
    end
    @pattern = @locked_pattern if defined?(@locked_pattern)
  end

  def apply_net(data)
    @net_id = data[:id]
    @net_map_id = data[:map].to_i
    @net_pname = data[:pname].to_s
    @net_pname = "Player" if @net_pname.empty?
    @net_clothes = data[:clothes].to_s
    @net_hair = data[:hair].to_s
    @net_hat = data[:hat].to_s
    @net_hat2 = data[:hat2].to_s
    @net_state = data[:state].to_i

    nx = data[:x].to_i
    ny = data[:y].to_i
    ndir = data[:dir].to_i
    ndir = 2 if ndir <= 0
    npat = data[:pattern].to_i
    nspd = data[:speed].to_i
    nspd = 3 if nspd <= 0
    ncname = data[:cname].to_s
    ncname = "walk" if ncname.empty?

    begin
      if respond_to?(:moveto)
        moveto(nx, ny)
      else
        @x = nx
        @y = ny
        if defined?(Game_Map::REAL_RES_X)
          @real_x = nx * Game_Map::REAL_RES_X
          @real_y = ny * Game_Map::REAL_RES_Y
        else
          @real_x = nx * 128
          @real_y = ny * 128
        end
      end
    rescue
      @x = nx
      @y = ny
    end

    @direction = ndir
    @pattern = npat
    @locked_pattern = npat
    @move_speed = nspd
    begin
      @character_name = ncname
    rescue
      instance_variable_set(:@character_name, ncname)
    end
    @opacity = 255
    @through = true
    @walk_anime = true
  end
end

begin
  FGL_RemotePlayer_Test.install_hooks!
rescue
end
