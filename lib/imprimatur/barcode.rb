# frozen_string_literal: true

module Imprimatur
  # A barcode: QR Code, Data Matrix, EAN-13, EAN-8, UPC-A or Code 128, and the
  # GS1 forms of QR Code, Data Matrix and Code 128 (GS1-128). Built with
  # {Imprimatur.barcode}; draw it with {#svg} or {#png}, or place it in a
  # {Document} with {Document#barcode}. The class itself is native.
  #
  #   qr = Imprimatur.barcode(:qr, 'https://id.gs1.org/01/09506000134352')
  #   File.write('qr.svg', qr.svg(module_size: 4))
  #
  #   label = Imprimatur.barcode(:code128, '(00)106141412345678908', gs1: true)
  #   File.binwrite('label.png', label.png(module_size: 3))
  #
  #   Imprimatur.barcode(:ean13, '950600013435').text # => "9506000134352"
  class Barcode
    SYMBOLOGIES = %i[qr datamatrix ean13 ean8 upca code128].freeze

    # @param symbology [:qr, :datamatrix, :ean13, :ean8, :upca, :code128]
    # @param data [String] text; a GTIN's digits for EAN and UPC, the check
    #   digit added when left out; with +gs1: true+, a GS1 element string
    #   such as "(01)09506000134352(17)251231(10)AB12"
    # @param gs1 [Boolean] a GS1 QR Code, GS1 DataMatrix or GS1-128
    # @param level [:l, :m, :q, :h, nil] a QR Code's error correction, :m unless set
    # @return [Barcode]
    # @raise [Imprimatur::Error] data the symbology cannot hold, an invalid
    #   element string, or a wrong check digit
    # @raise [ArgumentError] an unknown symbology or level, or one that does not apply
    def self.encode(symbology, data, gs1: false, level: nil)
      __encode(symbology.to_s, String(data), gs1 ? true : false, level&.to_s)
    end

    private_class_method :__encode

    # @param module_size [Numeric] user units (pixels) a module, 0.01 to 1000
    # @param color [String] the bars, "#RRGGBB"
    # @param background [String, nil] "#RRGGBB", or nil for none
    # @param quiet_zone [Boolean] keep the blank margin scanners need
    # @param bar_height [Integer, nil] a linear code's bar height in modules;
    #   the symbology's nominal height unless set
    # @return [String] SVG markup
    # @raise [Imprimatur::Error] a module size or color out of range
    def svg(module_size: 4, color: '#000000', background: '#FFFFFF', quiet_zone: true, bar_height: nil)
      __svg(Float(module_size), look(color, background, quiet_zone, bar_height))
    end

    # @param module_size [Integer] pixels a module
    # @param (see #svg)
    # @return [String] the PNG bytes (binary): one bit a pixel, the background
    #   transparent when nil
    # @raise [Imprimatur::Error] a size of 0, an image above 65 536 pixels a side,
    #   or an invalid color
    def png(module_size: 4, color: '#000000', background: '#FFFFFF', quiet_zone: true, bar_height: nil)
      __png(Integer(module_size), look(color, background, quiet_zone, bar_height))
    end

    # @!method symbology
    #   @return [Symbol] one of {SYMBOLOGIES}
    # @!method text
    #   @return [String] what it encodes: the data given, or a GTIN with its check digit
    # @!method gs1?
    #   @return [Boolean] whether it carries a GS1 element string
    # @!method width
    #   @return [Integer] modules across, without the quiet zone
    # @!method height
    #   @return [Integer] modules down; 1 for a linear code
    # @!method modules
    #   @return [Array<Array<Boolean>>] rows of modules, true for dark

    def inspect = "#<#{self.class} #{symbology}#{' gs1' if gs1?} #{text.inspect}>"

    private

    def look(color, background, quiet_zone, bar_height)
      [color.to_s, background&.to_s, quiet_zone ? true : false, bar_height && Integer(bar_height)]
    end
  end
end
