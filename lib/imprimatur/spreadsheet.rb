# frozen_string_literal: true

require 'date'
require 'json'

module Imprimatur
  # No row among the first ones of a sheet holds every header
  # {Spreadsheet::Sheet#parse} looks for.
  class HeaderRowNotFound < Error
    # @return [Array<String>] the headers missing from the row that holds the most of them
    attr_reader :missing

    def initialize(missing)
      @missing = missing.freeze
      super("no header row has #{missing.join(', ')}")
    end
  end

  # A workbook read for its values: +.xlsx+, +.xls+ (Excel 97-2003, and
  # Excel 5.0/95) or +.ods+. Values come as Ruby has them: Strings,
  # Integers for whole numbers shown without a decimal point or a percent
  # sign, Floats, true and false, Dates, and DateTimes for a time of day;
  # a formula without a computed result is nil.
  #
  #   book = Imprimatur::Spreadsheet.open('suppliers.xlsx')
  #   book.names                    # => ["Suppliers", "Countries"]
  #   book.sheet.rows.first         # => ["Name", "Country", "Since"]
  #   book.sheet.parse(name: 'Name', since: 'Since')
  #   # => [{ name: "Acme", since: #<Date: 2024-03-15> }, ...]
  class Spreadsheet
    # One sheet's values.
    class Sheet
      # How many rows from the top {#parse} searches for the header row.
      HEADER_ROWS = 100

      # @return [String]
      attr_reader :name

      # @return [Array<Array>] every row from the first to the last with a
      #   value, each an Array of values by column, nil where a cell is
      #   empty; a row without a value is an empty Array
      attr_reader :rows

      # @api private
      def initialize(name, hidden, rows, details)
        @name = name
        @hidden = hidden
        @rows = rows
        @details = details
      end

      def hidden? = @hidden

      # @return [Array<Hash>] +{cell:, author:, text:}+
      def comments = details[:comments]

      # @return [Array<Hash>] runs of +{first:, last:, width:, hidden:}+ columns, by letter
      def columns = details[:columns]

      # @return [Array<Hash>] +{range:, type:, formula1:, ...}+, as {Imprimatur.content} gives them
      def validations = details[:validations]

      # @return [Array<Hash>] each formatted cell's +{cell:, font:, size:, bold:, fill:, ...}+;
      #   empty unless the workbook was opened with +styles: true+
      def styles = details.fetch(:styles, [])

      # @return [Hash, nil] the validation over exactly +range+ (+"G2:G1000"+)
      def validation(range) = validations.find { it[:range] == range }

      # @return [Hash, nil] the look of the cell at +reference+ (+"A1"+)
      def style(reference) = styles.find { it[:cell] == reference }

      # The rows under the header row, as Hashes of +headers+' keys to the
      # values in the columns their headers name. The header row is the
      # first, among the first +header_rows+, to hold every header.
      #
      #   sheet.parse(clean: true, name: 'Company Name', country: 'Country')
      #   # => [{ name: "Acme", country: "PL" }, ...]
      #
      # @param clean [Boolean] strips control characters, and whitespace at
      #   both ends, from every String, headers included
      # @param header_rows [Integer]
      # @param headers [Hash{Symbol => String}]
      # @return [Array<Hash>]
      # @raise [HeaderRowNotFound] no row holds every header
      def parse(clean: false, header_rows: HEADER_ROWS, **headers)
        rows = clean ? @rows.map { |row| row.map { clean(it) } } : @rows
        header, columns = header_row(rows.first(header_rows), headers.values)
        rows.drop(header + 1).map do |row|
          headers.keys.zip(columns).to_h { |key, column| [key, row[column]] }
        end
      end

      private

      # The index of the first row holding every header, and their columns.
      def header_row(rows, headers)
        closest = []
        rows.each_with_index do |row, index|
          found = headers.filter_map { row.index(it) }
          return [index, found] if found.size == headers.size

          closest = headers.select { row.include?(it) } if found.size > closest.size
        end
        raise HeaderRowNotFound, headers - closest
      end

      def clean(value)
        value.is_a?(String) ? value.gsub(/[[:cntrl:]]|\A[[:space:]]+|[[:space:]]+\z/, '') : value
      end

      def details
        @details = JSON.parse(@details, symbolize_names: true) if @details.is_a?(String)
        @details
      end
    end

    # @param document [String, Pathname, #read] a path, the bytes or an IO
    # @param styles [Boolean] each sheet's {Sheet#styles} too
    # @param limits [Hash] +max_bytes:+, +max_inflated:+ and +timeout:+, as
    #   {Imprimatur.content} takes them
    # @return [Spreadsheet]
    # @raise [UnsupportedDocument] a PDF, a Word document or another format
    # @raise [Imprimatur::Error] a damaged workbook, or past a limit
    def self.open(document, styles: false, **limits)
      new(Native.sheets(Reader.read(document), *Reader.limits(**limits), styles ? true : false))
    end

    # @return [Array<Sheet>] in the workbook's order, hidden ones included
    attr_reader :sheets

    # @api private
    def initialize(sheets)
      @sheets = sheets.map { |name, hidden, rows, details| Sheet.new(name, hidden, rows, details) }.freeze
    end

    # @return [Array<String>]
    def names = sheets.map(&:name)

    # The sheet named +name+, or the first one.
    #
    # @return [Sheet]
    # @raise [KeyError] the workbook has no sheet of that name
    def sheet(name = nil)
      return sheets.first unless name

      sheets.find { it.name == name } or
        raise KeyError.new("no sheet named #{name.inspect}, only #{names.inspect}", receiver: self, key: name)
    end
  end
end
