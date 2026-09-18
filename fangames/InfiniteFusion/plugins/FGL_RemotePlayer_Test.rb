# =============================================================================
# FGL_RemotePlayer_Test.rb — PROTOTYPE EXPERIMENTAL (isolé)
# =============================================================================
# Objectif : simuler un joueur distant comme vrai Game_Character +
#            Sprite_Character natif, via IPC launcher uniquement.
#
# - Ne modifie PAS FGL_Net / Trade / Battle
# - Pas de FGL_peers
# - map_id distant N'IMPOSE PAS la map locale (pas de téléport, pas de filtre
#   d'affichage sur map_id)
# - Supprimer ce fichier = fin du prototype
# =============================================================================

module FGL_RemotePlayer_Test
  TICK = 0.05
  STALE_MISS = 120

  @sock = nil
  @my_id = nil
  @last = 0.0
  @remotes = {}
  @hooks_done = false

  def self.log(_msg); end

  def self.ipc_port
    p = ENV["FGL_IPC_PORT"].to_s.to_i
    return 0 if p < 1 || p > 65535
    p
  end

  def self.ensure_sock
    return if @sock
    begin
      require "socket"
      @sock = UDPSocket.new
      @sock.bind("127.0.0.1", 0)
    rescue
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
    rescue
    end
  end

  def self.ipc_poll
    ensure_sock
    return [] unless @sock
    out = []
    32.times do
      begin
        if @sock.respond_to?(:recvfrom_nonblock)
          data, _ = @sock.recvfrom_nonblock(65535)
          out << data.to_s if data
        else
          break
        end
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

  def self.write_local
    return unless $game_player && $game_map
    ensure_id
    p = $game_player
    cname = clean_name((p.character_name rescue "walk"))
    px = p.x.to_i rescue 0
    py = p.y.to_i rescue 0
    pdir = p.direction.to_i rescue 2
    pspeed = p.move_speed.to_i rescue 3
    ppat = p.pattern.to_i rescue 0
    mid = ($game_map.map_id rescue 0).to_i
    pname = "Player"
    begin
      pname = safe_text($player.name) if defined?($player) && $player && $player.name
    rescue
    end
    begin
      pname = safe_text($Trainer.name) if pname == "Player" && defined?($Trainer) && $Trainer && $Trainer.name
    rescue
    end
    line = [
      @my_id, "P", mid, px, py, pdir, cname, pspeed, ppat,
      0, 0, 0, 0, 0, 0, "", pname
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
      :pname => (a.size > 16 ? safe_text(a[16]) : "Player")
    }
  end

  def self.ingest_network
    ensure_id
    seen = {}
    ipc_poll.each do |raw|
      data = parse_player_raw(raw)
      next unless data
      id = data[:id]
      seen[id] = true
      rec = @remotes[id]
      if rec.nil?
        char = FGL_RemoteCharacter.new
        char.apply_net(data)
        spr = nil
        begin
          v = map_viewport
          if v && defined?(Sprite_Character)
            spr = Sprite_Character.new(v, char)
          end
        rescue
          spr = nil
        end
        @remotes[id] = { :char => char, :sprite => spr, :miss => 0, :data => data }
      else
        rec[:char].apply_net(data)
        rec[:data] = data
        rec[:miss] = 0
        if rec[:sprite].nil? || (rec[:sprite].disposed? rescue true)
          begin
            v = map_viewport
            if v && defined?(Sprite_Character)
              rec[:sprite] = Sprite_Character.new(v, rec[:char])
            end
          rescue
          end
        end
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
      begin
        if rec[:sprite] && !(rec[:sprite].disposed? rescue true)
          rec[:sprite].update
        end
      rescue
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
    rescue
    end
    begin
      return unless $scene.is_a?(Scene_Map)
    rescue
      return
    end
    begin
      ingest_network
    rescue
    end
    begin
      update_sprites
    rescue
    end
  end

  def self.install_hooks!
    return if @hooks_done
    @hooks_done = true
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
      end
    rescue
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
      end
    rescue
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
  end
end

class FGL_RemoteCharacter < Game_Character
  attr_reader :net_id, :net_map_id, :net_pname

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
        @visible = true
      end
    end
    @net_id = nil
    @net_map_id = 0
    @net_pname = "Player"
    @through = true
    @move_frequency = 6
    @walk_anime = true
    @step_anime = false
    @direction_fix = false
  end

  def apply_net(data)
    @net_id = data[:id]
    @net_map_id = data[:map].to_i
    @net_pname = data[:pname].to_s
    @net_pname = "Player" if @net_pname.empty?

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