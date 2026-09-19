# =============================================================================
# FGL_RemotePlayer_Test.rb — PROTOTYPE EXPERIMENTAL (isole)
# =============================================================================
# Joueur distant via IPC launcher + rendu Infinite Fusion (pas Sprite_Character).
# Chemin: Game -> IPC (FGL_IPC_PORT) -> Launcher -> EasyTier -> Launcher -> IPC -> Game
# - Ne modifie PAS FGL_Net / Trade / Battle
# - Transport UDP propre (socket bind 127.0.0.1:0)
# - Rendu: build_body_bitmap / Sprite.new / couches / tile_to_screen (logique FGL_Net)
# =============================================================================

module FGL_RemotePlayer_Test
  TICK = 0.05
  STALE_MISS = 120
  STATUS_EVERY = 30
  FW = 80
  FH = 80

  @sock = nil
  @my_id = nil
  @last = 0.0
  @remotes = {}
  @hooks_done = false
  @status_n = 0
  @last_err = ""
  @poll_raw = 0

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
      lines << "remotes=#{@remotes.size}"
      lines << "scene=#{$scene ? $scene.class.name : "nil"}"
      lines << "vp=#{map_viewport ? "ok" : "nil"}"
      lines << "err=#{@last_err}"
      @remotes.each do |id, rec|
        lines << "remote id=#{id} map=#{rec[:map]} x=#{rec[:x]} y=#{rec[:y]} dir=#{rec[:dir]} cname=#{rec[:cname]} spr=#{rec[:sprite] ? "yes" : "no"}"
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
    return if @sock
    return if ipc_port <= 0
    begin
      require "socket"
      @sock = UDPSocket.new
      @sock.bind("127.0.0.1", 0)
    rescue => e
      @last_err = "sock:#{e}"
      @sock = nil
    end
  end

  def self.ipc_send(line)
    p = ipc_port
    return if p <= 0
    ensure_sock
    return unless @sock
    begin
      @sock.send("PLAYER|" + line.to_s, 0, "127.0.0.1", p)
    rescue => e
      @last_err = "send:#{e}"
    end
  end

  def self.ipc_poll
    return [] if ipc_port <= 0
    ensure_sock
    return [] unless @sock
    out = []
    32.times do
      begin
        data = nil
        if @sock.respond_to?(:recvfrom_nonblock)
          begin
            data, _addr = @sock.recvfrom_nonblock(65535)
          rescue Errno::EAGAIN, Errno::EWOULDBLOCK
            break
          rescue IO::WaitReadable
            break
          end
        else
          ready = true
          begin
            if defined?(IO) && IO.respond_to?(:select)
              r = IO.select([@sock], nil, nil, 0)
              ready = r && r[0] && r[0].include?(@sock)
            end
          rescue
            ready = true
          end
          break unless ready
          data, _addr = @sock.recvfrom(65535)
        end
        out << data.to_s if data && data.to_s.length > 0
      rescue Errno::EAGAIN, Errno::EWOULDBLOCK
        break
      rescue
        break
      end
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

  def self.state_to_action(state, cname)
    cn = cname.to_s.downcase
    return "surf" if state.to_i == 1 || cn.include?("surf")
    return "dive" if state.to_i == 4 || cn.include?("dive")
    return "bike" if state.to_i == 2 || cn.include?("bike")
    return "fish" if state.to_i == 3 || cn.include?("fish")
    return "run" if cn.include?("run")
    "walk"
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

  # ---------- Rendu Infinite Fusion (repris de FGL_Net) ----------

  def self.safe_dispose(obj)
    return unless obj
    begin
      obj.dispose unless (obj.disposed? rescue false)
    rescue
    end
  end

  def self.destroy_player_visuals(rec)
    return unless rec
    [:sprite, :hair_spr, :hat_spr, :hat2_spr, :bike_spr, :surf_sprite].each do |k|
      safe_dispose(rec[k])
      rec[k] = nil
    end
    [:owned_bmp, :hair_bmp, :hat_bmp, :hat2_bmp, :bike_bmp].each do |k|
      begin
        rec[k].dispose if rec[k]
      rescue
      end
      rec[k] = nil
    end
    begin
      rec[:surf_anim].dispose if rec[:surf_anim] && rec[:surf_anim].respond_to?(:dispose)
    rescue
    end
    rec[:surf_anim] = nil
    rec[:bound_map_id] = nil
    rec[:last_outfit_key] = nil
    rec[:frozen_sx] = nil
    rec[:frozen_sy] = nil
  end

  def self.tile_to_screen(tx, ty, map = nil)
    map = $game_map if map.nil?
    begin
      if defined?(Game_Map::REAL_RES_X) && defined?(Game_Map::X_SUBPIXELS)
        sx = ((tx * Game_Map::REAL_RES_X - map.display_x) / Game_Map::X_SUBPIXELS).ceil
        sy = ((ty * Game_Map::REAL_RES_Y - map.display_y) / Game_Map::Y_SUBPIXELS).ceil
        tw = defined?(Game_Map::TILE_WIDTH) ? Game_Map::TILE_WIDTH : 32
        th = defined?(Game_Map::TILE_HEIGHT) ? Game_Map::TILE_HEIGHT : 32
        return [sx + tw / 2, sy + th]
      end
    rescue
    end
    begin
      return [(tx * 32) - (map.display_x / 4) + 16,
              (ty * 32) - (map.display_y / 4) + 32]
    rescue
      return [0, 0]
    end
  end

  def self.calc_z(sy, remote_y = nil)
    z = sy + 32
    begin
      if remote_y && $game_player
        if remote_y > $game_player.y
          z += 20
        elsif remote_y < $game_player.y
          z -= 20
        end
      end
    rescue
    end
    z
  end

  def self.offset_for(action, dir, frame)
    f = frame.to_i; f = 0 if f < 0; f = 3 if f > 3
    d = dir.to_i; d = 2 if d <= 0
    case action.to_s
    when "surf"
      table = {2 => [[0,-6],[0,-4],[0,-6],[0,-4]], 4 => [[-2,-10],[-2,-8],[-2,-10],[-2,-8]],
               6 => [[2,-10],[2,-8],[2,-10],[2,-8]], 8 => [[0,-10],[0,-8],[0,-10],[0,-8]]}
      f = 0
    when "dive"
      table = {2 => [[0,-6],[0,-4],[0,-6],[0,-4]], 4 => [[6,-8],[6,-6],[6,-8],[6,-6]],
               6 => [[-6,-8],[-6,-6],[-6,-8],[-6,-6]], 8 => [[0,-2],[0,0],[0,-2],[0,0]]}
    when "bike"
      table = {2 => [[0,-2],[2,0],[0,-2],[-2,0]], 4 => [[-4,-4],[-2,-2],[-4,-4],[-6,-2]],
               6 => [[4,-4],[2,-2],[4,-4],[6,-2]], 8 => [[0,-2],[-2,0],[0,-2],[2,0]]}
    when "fish"
      table = {2 => [[0,-6],[0,-2],[0,-8],[2,-6]], 4 => [[0,-8],[-6,-6],[0,-8],[2,-8]],
               6 => [[0,-8],[6,-6],[0,-8],[-2,-8]], 8 => [[0,-6],[0,-6],[0,-6],[2,-4]]}
    when "run"
      table = {2 => [[0,2],[0,6],[0,2],[0,6]], 4 => [[-2,-2],[-2,-2],[-2,-2],[-2,-2]],
               6 => [[2,-2],[2,-2],[2,-2],[2,-2]], 8 => [[0,-2],[0,-2],[0,-2],[0,-2]]}
    else
      return [0, 0]
    end
    arr = table[d] || table[2]
    arr ? arr[f] : [0, 0]
  end

  def self.apply_tint(spr)
    return unless spr
    begin
      pbDayNightTint(spr) if defined?(pbDayNightTint)
    rescue
    end
  end

  def self.build_body_bitmap(rec)
    action = state_to_action(rec[:state], rec[:cname])
    begin
      base_path = nil
      if defined?(getBaseOverworldSpriteFilename)
        base_path = getBaseOverworldSpriteFilename(action, rec[:skin].to_i) rescue nil
      end
      if !base_path || (defined?(pbResolveBitmap) && !pbResolveBitmap(base_path))
        base_path = Settings::PLAYER_GRAPHICS_FOLDER + action if defined?(Settings::PLAYER_GRAPHICS_FOLDER)
      end
      if !base_path || (defined?(pbResolveBitmap) && !pbResolveBitmap(base_path))
        base_path = "Graphics/Characters/#{action}"
      end
      base = AnimatedBitmap.new(base_path)
      out = base.bitmap.clone
      if defined?(getOverworldOutfitFilename)
        op = getOverworldOutfitFilename(rec[:clothes], action) rescue nil
        if (!op || !pbResolveBitmap(op)) && defined?(Settings::PLAYER_TEMP_OUTFIT_FALLBACK)
          op = getOverworldOutfitFilename(Settings::PLAYER_TEMP_OUTFIT_FALLBACK) rescue op
        end
        if op && pbResolveBitmap(op)
          ob = AnimatedBitmap.new(op, rec[:cc].to_i)
          out.blt(0, 0, ob.bitmap, ob.bitmap.rect)
        end
      end
      return out
    rescue
    end
    begin
      cn = rec[:cname].to_s
      cn = "walk" if cn.empty?
      return AnimatedBitmap.new("Graphics/Characters/#{cn}").bitmap.clone
    rescue
      return nil
    end
  end

  def self.load_layer_bitmap(path, hue)
    return nil if path.nil? || path.to_s == "" || path.to_s == "0"
    begin
      return nil if defined?(pbResolveBitmap) && !pbResolveBitmap(path)
      return AnimatedBitmap.new(path, hue.to_i).bitmap.clone
    rescue
      return nil
    end
  end

  def self.build_surfmon_anim(rec)
    begin
      species = nil
      if rec[:surfmon] && rec[:surfmon] != ""
        begin
          species = GameData::Species.get(rec[:surfmon].to_sym) if defined?(GameData::Species)
        rescue
        end
      end
      basePath = ""
      begin
        if defined?(Settings::PLAYER_GRAPHICS_FOLDER)
          basePath = Settings::PLAYER_GRAPHICS_FOLDER.to_s
          basePath += Settings::PLAYER_SURFBASE_FOLDER.to_s if Settings.const_defined?(:PLAYER_SURFBASE_FOLDER)
        end
      rescue
      end
      is_dive = (rec[:state].to_i == 4)
      act = is_dive ? "divemon" : "surfmon"
      candidates = []
      if species && species.respond_to?(:shape)
        candidates << "#{basePath}#{act}_#{species.shape.to_s}"
      end
      candidates << "#{basePath}#{act}_Head"
      candidates << "#{basePath}surfmon_board"
      candidates.each do |p|
        next if p.nil? || p == ""
        begin
          ok = defined?(pbResolveBitmap) ? pbResolveBitmap(p) : true
          return AnimatedBitmap.new(p) if ok
        rescue
        end
      end
    rescue
    end
    nil
  end

  def self.make_layer_sprite(v, bmp, z)
    return nil unless bmp
    s = ::Sprite.new(v)
    s.bitmap = bmp
    s.visible = true
    s.opacity = 255
    s.z = z
    apply_tint(s)
    s
  end

  def self.ensure_sprite(rec)
    return unless rec
    begin
      return unless $game_map && $scene.is_a?(Scene_Map)
    rescue
      return
    end
    remote_mid = rec[:map].to_i
    local_mid = ($game_map.map_id rescue 0).to_i
    if remote_mid != 0 && local_mid != 0 && remote_mid != local_mid
      destroy_player_visuals(rec) if rec[:sprite]
      return
    end
    v = map_viewport
    return if v.nil?

    action = state_to_action(rec[:state], rec[:cname])
    outfit_key = [rec[:cname], rec[:clothes], rec[:hair], rec[:hat], rec[:hat2],
                  rec[:cc], rec[:hc], rec[:htc], rec[:h2c], rec[:skin],
                  rec[:state], rec[:surfmon], rec[:bike_col]].join("|")
    need = !rec[:sprite]
    if rec[:sprite]
      begin
        need = true if rec[:sprite].disposed?
      rescue
        need = true
      end
    end
    if rec[:bound_map_id] != remote_mid || (rec[:last_outfit_key] && rec[:last_outfit_key] != outfit_key)
      destroy_player_visuals(rec)
      need = true
    end

    if need
      destroy_player_visuals(rec)
      body = build_body_bitmap(rec)
      if body.nil?
        @last_err = "body_bmp_nil"
        return
      end

      s = make_layer_sprite(v, body, 100)
      begin
        s.ox = FW / 2
        s.oy = FH
        dir = rec[:dir].to_i; dir = 2 if dir <= 0
        pat = rec[:pattern].to_i
        s.src_rect.set(pat * FW, ((dir - 2) / 2) * FH, FW, FH)
      rescue
      end
      rec[:sprite] = s
      rec[:owned_bmp] = body

      if defined?(getOverworldHairFilename) && rec[:hair].to_s != "" && rec[:hair].to_s != "0"
        hp = getOverworldHairFilename(rec[:hair]) rescue nil
        hb = load_layer_bitmap(hp, rec[:hc])
        if hb
          hs = make_layer_sprite(v, hb, 101)
          begin
            hs.ox = FW / 2; hs.oy = FH
          rescue
          end
          rec[:hair_spr] = hs
          rec[:hair_bmp] = hb
        end
      end

      [[:hat2, :h2c, :hat2_spr, :hat2_bmp, 102],
       [:hat,  :htc, :hat_spr,  :hat_bmp,  103]].each do |idk, colk, sprk, bmpk, zz|
        hid = rec[idk].to_s
        next if hid == "" || hid == "0"
        next unless defined?(getOverworldHatFilename)
        hpath = getOverworldHatFilename(hid) rescue nil
        hbm = load_layer_bitmap(hpath, rec[colk])
        if hbm
          hs = make_layer_sprite(v, hbm, zz)
          begin
            hs.ox = FW / 2; hs.oy = FH
          rescue
          end
          rec[sprk] = hs
          rec[bmpk] = hbm
        end
      end

      if action == "bike" && defined?(getOverworldBicycleFilename)
        begin
          bp = getOverworldBicycleFilename rescue nil
          bb = load_layer_bitmap(bp, rec[:bike_col])
          if bb
            bs = make_layer_sprite(v, bb, 99)
            begin
              bs.ox = FW / 2; bs.oy = FH
            rescue
            end
            rec[:bike_spr] = bs
            rec[:bike_bmp] = bb
          end
        rescue
        end
      end

      if rec[:state].to_i == 1 || rec[:state].to_i == 4
        sanim = build_surfmon_anim(rec)
        if sanim
          sb = sanim.bitmap
          ss = make_layer_sprite(v, sb, 98)
          begin
            cw = sb.width / 4
            ch = sb.height / 4
            dir = rec[:dir].to_i; dir = 2 if dir <= 0
            pat = rec[:pattern].to_i
            ss.src_rect.set(pat * cw, ((dir - 2) / 2) * ch, cw, ch)
            ss.ox = cw / 2
            ss.oy = ch - 16
          rescue
          end
          rec[:surf_sprite] = ss
          rec[:surf_anim] = sanim
        end
      end

      rec[:bound_map_id] = remote_mid
      rec[:last_outfit_key] = outfit_key
    end
    update_sprite_pos(rec)
  end

  def self.update_sprite_pos(rec)
    s = rec[:sprite]
    return unless s
    begin
      sx, sy = tile_to_screen(rec[:x].to_i, rec[:y].to_i, $game_map)
      dir = rec[:dir].to_i; dir = 2 if dir <= 0
      pat = rec[:pattern].to_i
      action = state_to_action(rec[:state], rec[:cname])
      body_sy = sy
      body_sy = sy + 16 if action == "surf" || action == "dive"
      s.x = sx
      s.y = body_sy
      base_z = calc_z(sy, rec[:y])
      mon_bob = 0
      begin
        mon_bob = ((Graphics.frame_count / 10) % 2) if action == "surf" || action == "dive"
      rescue
      end
      if s.bitmap
        w = (s.bitmap.width >= 4 * FW) ? FW : (s.bitmap.width / 4)
        h = (s.bitmap.height >= 4 * FH) ? FH : (s.bitmap.height / 4)
        w = 1 if w < 1; h = 1 if h < 1
        s.src_rect.set(pat * w, ((dir - 2) / 2) * h, w, h)
        s.ox = w / 2
        s.oy = h
        s.oy -= mon_bob if mon_bob != 0
      end
      s.z = base_z
      apply_tint(s)
      s.visible = true
      if rec[:surf_sprite]
        ss = rec[:surf_sprite]
        ss.x = sx
        ss.y = sy
        if ss.bitmap
          cw = ss.bitmap.width / 4
          ch = ss.bitmap.height / 4
          ss.src_rect.set(pat * cw, ((dir - 2) / 2) * ch, cw, ch)
          ss.ox = cw / 2
          ss.oy = ch - 16
          ss.oy -= mon_bob if mon_bob != 0
        end
        ss.z = base_z - 2
        apply_tint(ss)
        ss.visible = true
      end
      if rec[:bike_spr]
        bs = rec[:bike_spr]
        bs.x = s.x
        bs.y = s.y
        bs.ox = s.ox
        bs.oy = s.oy
        if bs.bitmap
          bs.src_rect.set(pat * FW, ((dir - 2) / 2) * FH, FW, FH)
        end
        bs.z = base_z - 1
        apply_tint(bs)
        bs.visible = true
      end
      ox, oy = offset_for(action, dir, pat)
      extra_y = 0
      extra_y = -2 if (pat % 2 == 1) && action != "surf"
      [[:hair_spr, 1, true],
       [:hat2_spr, 2, false],
       [:hat_spr,  3, false]].each do |key, add, is_hair|
        spr = rec[key]
        next unless spr
        spr.x = s.x + ox
        spr.y = s.y + oy + extra_y
        spr.ox = s.ox
        spr.oy = s.oy
        if spr.bitmap
          fx = pat
          fx = 0 if is_hair && action == "surf"
          if is_hair || spr.bitmap.width >= 4 * FW
            spr.src_rect.set(fx * FW, ((dir - 2) / 2) * FH, FW, FH)
          else
            spr.src_rect.set(0, ((dir - 2) / 2) * FH, [spr.bitmap.width, FW].min, FH)
          end
        end
        spr.z = base_z + add
        apply_tint(spr)
        spr.visible = true
      end
    rescue => e
      @last_err = "pos:#{e}"
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
        rec = {
          :sprite => nil, :hair_spr => nil, :hat_spr => nil, :hat2_spr => nil,
          :bike_spr => nil, :surf_sprite => nil, :surf_anim => nil,
          :owned_bmp => nil, :hair_bmp => nil, :hat_bmp => nil, :hat2_bmp => nil, :bike_bmp => nil,
          :bound_map_id => nil, :last_outfit_key => nil, :miss => 0
        }
        @remotes[id] = rec
      end
      rec[:map] = data[:map]
      rec[:x] = data[:x]
      rec[:y] = data[:y]
      rec[:dir] = data[:dir]
      rec[:cname] = data[:cname]
      rec[:speed] = data[:speed]
      rec[:pattern] = data[:pattern]
      rec[:action] = data[:action]
      rec[:pname] = data[:pname]
      rec[:clothes] = data[:clothes]
      rec[:hair] = data[:hair]
      rec[:hat] = data[:hat]
      rec[:hat2] = data[:hat2]
      rec[:cc] = data[:cc]
      rec[:hc] = data[:hc]
      rec[:htc] = data[:htc]
      rec[:h2c] = data[:h2c]
      rec[:skin] = data[:skin]
      rec[:state] = data[:state]
      rec[:surfmon] = data[:surfmon]
      rec[:bike_col] = data[:bike_col]
      rec[:miss] = 0
      ensure_sprite(rec)
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
    destroy_player_visuals(rec)
    @remotes.delete(id)
  end

  def self.update_sprites
    @remotes.each_value do |rec|
      ensure_sprite(rec)
      begin
        update_sprite_pos(rec) if rec[:sprite]
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

begin
  FGL_RemotePlayer_Test.install_hooks!
rescue
end